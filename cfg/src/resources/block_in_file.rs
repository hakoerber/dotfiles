use anyhow::{Result, anyhow, bail};
use camino::Utf8Path as Path;
use regex::Regex;

use super::super::{
    Applyable, ChangeStatus, Checksum, Context, Plannable, Reporter, Tempfile, checksum, fs,
    fs::RawFileContent, maybeowned::MaybeOwned,
};

#[derive(Debug)]
pub struct BlockInFile<'change> {
    pub path: &'change Path,
    pub block: MaybeOwned<'change, [MaybeOwned<'change, str>]>,
    pub marker: &'change str,
    pub insert_after: Option<Regex>,
}

impl<'change> Plannable<'change> for BlockInFile<'change> {
    fn plan<'ctx>(self: Box<Self>, ctx: &'ctx Context<'ctx>) -> Result<ChangeStatus<'change>>
    where
        'ctx: 'change,
    {
        let start_marker = format!("{marker} --- START", marker = self.marker);
        let end_marker = format!("{marker} --- END", marker = self.marker);

        let content = fs::read_to_string(&ctx.elevate, self.path)?;

        let new_content = {
            enum State {
                Initial,
                Before,
                In,
                After,
            }

            let mut new_content = RawFileContent::new();

            let mut state = State::Initial;

            for line in content.lines() {
                if line == start_marker {
                    match state {
                        State::Before | State::Initial => {
                            new_content.write_all(start_marker.as_bytes());
                            new_content.write_all(b"\n");
                            for line in self.block.as_ref() {
                                new_content.write_all(line.as_ref().as_bytes());
                                new_content.write_all(b"\n");
                            }
                            // we could write end marker here, but it feels wrong
                            state = State::In;
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
                            new_content.write_all(end_marker.as_bytes());
                            new_content.write_all(b"\n");
                            state = State::After;
                        }
                        State::After => {
                            bail!("found another end marker after end");
                        }
                    }
                } else {
                    match state {
                        State::Before | State::Initial => {
                            new_content.write_all(line.as_bytes());
                            new_content.write_all(b"\n");
                            state = State::Before;
                        }
                        State::In => {
                            // ignore lines that are in the existing block
                        }
                        State::After => {
                            new_content.write_all(line.as_bytes());
                            new_content.write_all(b"\n");
                        }
                    }
                }
            }

            // check once we went through the whole file, we may have to add the block
            match state {
                State::Before | State::Initial => {
                    // marker was never found, we have to find the right place to put it
                    if let Some(ref insert_after) = self.insert_after {
                        // specific insertion place, we need a second pass
                        enum State {
                            Found,
                            NotFound,
                        }

                        let mut state = State::NotFound;

                        // start from scratch
                        new_content.truncate();

                        for line in content.lines() {
                            if insert_after.is_match(line) {
                                match state {
                                    State::Found => bail!("insert_after found more than once"),
                                    State::NotFound => {
                                        state = State::Found;
                                        new_content.write_all(line.as_bytes());
                                        new_content.write_all(b"\n");
                                        new_content.write_all(start_marker.as_bytes());
                                        new_content.write_all(b"\n");
                                        for line in self.block.as_ref() {
                                            new_content.write_all(line.as_ref().as_bytes());
                                            new_content.write_all(b"\n");
                                        }
                                        new_content.write_all(end_marker.as_bytes());
                                        new_content.write_all(b"\n");
                                    }
                                }
                            } else {
                                new_content.write_all(line.as_bytes());
                                new_content.write_all(b"\n");
                            }
                        }

                        match state {
                            State::Found => (),
                            State::NotFound => {
                                // regex never found, add to the end
                                new_content.write_all(start_marker.as_bytes());
                                new_content.write_all(b"\n");
                                for line in self.block.as_ref() {
                                    new_content.write_all(line.as_ref().as_bytes());
                                    new_content.write_all(b"\n");
                                }
                                new_content.write_all(end_marker.as_bytes());
                                new_content.write_all(b"\n");
                            }
                        }
                    } else {
                        // just put it at the end
                        new_content.write_all(start_marker.as_bytes());
                        new_content.write_all(b"\n");
                        for line in self.block.as_ref() {
                            new_content.write_all(line.as_ref().as_bytes());
                            new_content.write_all(b"\n");
                        }
                        new_content.write_all(end_marker.as_bytes());
                        new_content.write_all(b"\n");
                    }
                }
                State::In => {
                    bail!("no end marker found");
                }
                State::After => (),
            }

            new_content
        };

        let (checksum_from, checksum_to) = (
            checksum::checksum_from(new_content.as_bytes()),
            checksum::checksum_from(content.as_bytes()),
        );

        if checksum_from != checksum_to {
            Ok(ChangeStatus::NeedsChange(Box::new(BlockInFilePlan {
                ctx,
                args: *self,
                new_content,
                existing_content: RawFileContent::from_string(content),
                checksum_from,
                checksum_to,
            })))
        } else {
            Ok(ChangeStatus::Clean)
        }
    }
}

#[derive(Debug)]
struct BlockInFilePlan<'a> {
    ctx: &'a Context<'a>,
    args: BlockInFile<'a>,
    existing_content: RawFileContent,
    new_content: RawFileContent,
    checksum_from: Checksum,
    checksum_to: Checksum,
}

impl Applyable for BlockInFilePlan<'_> {
    fn apply(&self) -> Result<()> {
        let tmpfile = Tempfile::new(&self.ctx.elevate)?;
        tmpfile.write(self.new_content.as_bytes())?;
        tmpfile.save_as(self.args.path)?;
        Ok(())
    }
}

impl Reporter for BlockInFilePlan<'_> {
    fn report(&self) -> Result<Vec<String>> {
        let mut out = vec![format!(
            "checksum changed: {} -> {}",
            self.checksum_from, self.checksum_to
        )];

        let diff = diffy::create_patch_bytes(
            self.existing_content.as_bytes(),
            self.new_content.as_bytes(),
        );

        let formatter = diffy::PatchFormatter::new().with_color();
        let mut buffer = Vec::<u8>::new();
        formatter.write_patch_into(&diff, &mut buffer)?;

        let diff = String::from_utf8(buffer).map_err(|_err| anyhow!("non utf-8 patch"))?;
        out.extend(diff.lines().map(|s| s.to_owned()).collect::<Vec<String>>());

        Ok(out)
    }
}

#[derive(Debug)]
pub struct LineInFile<'change> {
    pub path: &'change Path,
    pub line: MaybeOwned<'change, str>,
    pub regex: Regex,
}

impl<'change> Plannable<'change> for LineInFile<'change> {
    fn plan<'ctx>(self: Box<Self>, ctx: &'ctx Context<'ctx>) -> Result<ChangeStatus<'change>>
    where
        'ctx: 'change,
    {
        let content = fs::read_to_string(&ctx.elevate, self.path)?;

        let new_content = {
            enum State {
                Initial,
                Found { at: usize },
            }

            let mut new_content = RawFileContent::new();

            let mut state = State::Initial;

            for (i, line) in content.lines().enumerate() {
                if self.regex.is_match(line) {
                    state = State::Found { at: i };
                }
            }

            match state {
                State::Initial => bail!("regex not found"),
                State::Found { at } => {
                    for (i, line) in content.lines().enumerate() {
                        if i == at {
                            new_content.write_all(self.line.as_ref().as_bytes());
                        } else {
                            new_content.write_all(line.as_bytes());
                        }
                        new_content.write_all(b"\n");
                    }
                }
            }

            new_content
        };

        let (checksum_from, checksum_to) = (
            checksum::checksum_from(new_content.as_bytes()),
            checksum::checksum_from(content.as_bytes()),
        );

        if checksum_from != checksum_to {
            Ok(ChangeStatus::NeedsChange(Box::new(LineInFilePlan {
                ctx,
                args: *self,
                existing_content: RawFileContent::from_string(content),
                new_content,
                checksum_from,
                checksum_to,
            })))
        } else {
            Ok(ChangeStatus::Clean)
        }
    }
}

#[derive(Debug)]
struct LineInFilePlan<'a> {
    ctx: &'a Context<'a>,
    args: LineInFile<'a>,
    existing_content: RawFileContent,
    new_content: RawFileContent,
    checksum_from: Checksum,
    checksum_to: Checksum,
}

impl Applyable for LineInFilePlan<'_> {
    fn apply(&self) -> Result<()> {
        let tmpfile = Tempfile::new(&self.ctx.elevate)?;
        tmpfile.write(self.new_content.as_bytes())?;
        tmpfile.save_as(self.args.path)?;
        Ok(())
    }
}

impl Reporter for LineInFilePlan<'_> {
    fn report(&self) -> Result<Vec<String>> {
        let mut out = vec![format!(
            "checksum changed: {} -> {}",
            self.checksum_from, self.checksum_to
        )];

        let diff = diffy::create_patch_bytes(
            self.existing_content.as_bytes(),
            self.new_content.as_bytes(),
        );

        let formatter = diffy::PatchFormatter::new().with_color();
        let mut buffer = Vec::<u8>::new();
        formatter.write_patch_into(&diff, &mut buffer)?;

        let diff = String::from_utf8(buffer).map_err(|_err| anyhow!("non utf-8 patch"))?;
        out.extend(diff.lines().map(|s| s.to_owned()).collect::<Vec<String>>());

        Ok(out)
    }
}
