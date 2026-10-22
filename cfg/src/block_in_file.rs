use std::{
    env, fmt, fs,
    io::{self, Write},
    os::unix::fs::{self as unixfs, MetadataExt as _, PermissionsExt},
    process::{self},
};

use anyhow::{Result, anyhow, bail};
use camino::{Utf8Path as Path, Utf8PathBuf as PathBuf};
use nonempty::{NonEmpty, nonempty};
use regex::Regex;
use sha3::Digest as _;
use tap::Tap as _;

use super::{
    Applyable, Change, ChangeStatus, Checksum, Context, Plannable, Reporter, Tempfile, checksum,
    checksum::checksum_file, move_file, tempfile,
};

pub(super) struct BlockInFile<'a> {
    pub(super) path: &'a Path,
    pub(super) block: &'a [&'a str],
    pub(super) marker: &'a str,
    pub(super) insert_after: Option<Regex>,
}

impl<'a> Plannable<'a> for BlockInFile<'a> {
    fn plan(self: Box<Self>, ctx: &'a Context<'a>) -> Result<ChangeStatus<'a>>
    where
        Self: Sized,
    {
        let mut new_content = FileContent::new();

        let start_marker = format!("{marker} --- START", marker = self.marker);
        let end_marker = format!("{marker} --- END", marker = self.marker);

        enum State {
            Initial,
            Before,
            In,
            After,
        }

        let mut state = State::Initial;

        let content = fs::read_to_string(self.path)?;

        for line in content.lines() {
            if line == start_marker {
                match state {
                    State::Before | State::Initial => {
                        new_content.write_all(start_marker.as_bytes())?;
                        new_content.write_all(b"\n")?;
                        for line in self.block {
                            new_content.write_all(line.as_bytes())?;
                            new_content.write_all(b"\n")?;
                        }
                        // we could write end marker here, but it feels wrong
                        state = State::In;
                        continue;
                    }
                    State::In => {
                        bail!("found two start markers");
                    }
                    State::After => {
                        bail!("found another start marker after end");
                    }
                }
            } else if line == end_marker {
                match state {
                    State::Before | State::Initial => {
                        bail!("found end marker before start marker")
                    }
                    State::In => {
                        new_content.write_all(end_marker.as_bytes())?;
                        new_content.write_all(b"\n")?;
                        state = State::After;
                        continue;
                    }
                    State::After => {
                        bail!("found another end marker after end");
                    }
                }
            } else {
                match state {
                    State::Before | State::Initial => {
                        new_content.write_all(line.as_bytes())?;
                        new_content.write_all(b"\n")?;
                        state = State::Before;
                        continue;
                    }
                    State::In => {
                        // ignore lines that are in the existing block
                        continue;
                    }
                    State::After => {
                        new_content.write_all(line.as_bytes())?;
                        new_content.write_all(b"\n")?;
                        continue;
                    }
                }
            }
        }

        match state {
            State::Before | State::Initial => {
                // marker was never found, we have to find the right place to put it
                match self.insert_after {
                    Some(ref insert_after) => {
                        // specific insertion place, we need a second pass

                        // start from scratch
                        new_content.truncate();

                        enum State {
                            Found,
                            NotFound,
                        }

                        let mut state = State::NotFound;

                        for line in content.lines() {
                            if insert_after.is_match(line) {
                                match state {
                                    State::Found => bail!("insert_after found more than once"),
                                    State::NotFound => {
                                        state = State::Found;
                                        new_content.write_all(line.as_bytes())?;
                                        new_content.write_all(b"\n")?;
                                        new_content.write_all(start_marker.as_bytes())?;
                                        new_content.write_all(b"\n")?;
                                        for line in self.block {
                                            new_content.write_all(line.as_bytes())?;
                                            new_content.write_all(b"\n")?;
                                        }
                                        new_content.write_all(end_marker.as_bytes())?;
                                        new_content.write_all(b"\n")?;
                                    }
                                }
                            } else {
                                new_content.write_all(line.as_bytes())?;
                                new_content.write_all(b"\n")?;
                            }
                        }

                        match state {
                            State::Found => (),
                            State::NotFound => {
                                println!("regex never found");
                                new_content.write_all(start_marker.as_bytes())?;
                                new_content.write_all(b"\n")?;
                                for line in self.block {
                                    new_content.write_all(line.as_bytes())?;
                                    new_content.write_all(b"\n")?;
                                }
                                new_content.write_all(end_marker.as_bytes())?;
                                new_content.write_all(b"\n")?;
                            }
                        }
                    }
                    None => {
                        // just put it at the end
                        new_content.write_all(start_marker.as_bytes())?;
                        new_content.write_all(b"\n")?;
                        for line in self.block {
                            new_content.write_all(line.as_bytes())?;
                            new_content.write_all(b"\n")?;
                        }
                        new_content.write_all(end_marker.as_bytes())?;
                        new_content.write_all(b"\n")?;
                    }
                }
            }
            State::Before | State::In => {
                bail!("no end marker found");
            }
            State::After => (),
        };

        let (checksum_from, checksum_to) = (
            checksum::checksum_from(new_content.as_bytes()),
            checksum::checksum_file(self.path)?.ok_or(anyhow!("file does not exist"))?,
        );

        if checksum_from != checksum_to {
            Ok(ChangeStatus::Changed(Box::new(BlockInFilePlan {
                ctx,
                args: *self,
                new_content,
                checksum_from,
                checksum_to,
            })))
        } else {
            Ok(ChangeStatus::Clean)
        }
    }
}

struct FileContent(Vec<u8>);

impl FileContent {
    fn new() -> Self {
        Self(vec![])
    }

    fn truncate(&mut self) {
        self.0.truncate(0);
    }

    fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl io::Write for FileContent {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

struct BlockInFilePlan<'a> {
    ctx: &'a Context<'a>,
    args: BlockInFile<'a>,
    new_content: FileContent,
    checksum_from: Checksum,
    checksum_to: Checksum,
}

impl Applyable for BlockInFilePlan<'_> {
    fn apply(&self) -> Result<()> {
        let mut tmpfile = Tempfile::new_in(self.ctx.cachedir)?;
        tmpfile.write_all(self.new_content.as_bytes())?;
        move_file(tmpfile.path(), self.args.path, &self.ctx.elevate)?;
        Ok(())
    }
}

impl Reporter for BlockInFilePlan<'_> {
    fn report(&self) {
        println!("{} -> {}", self.checksum_from, self.checksum_to);
    }
}
