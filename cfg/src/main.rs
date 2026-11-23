use std::{collections::HashMap, io, process};

use anyhow::{Result, anyhow};
use cfg::{migrations, root::remove_unconfigured_packages};
use clap::{Parser, Subcommand};

use maybeowned::prelude::*;
use oxidizer::prelude::*;

use cfg::{aur, hosts::Host, root, secret::Secrets, user};

#[derive(Subcommand)]
enum Action {
    Deploy {
        #[arg(long)]
        dry_run: bool,

        #[arg(long)]
        only_changed: bool,

        #[arg(long)]
        clean_packages: bool,

        #[arg(long)]
        secrets_stdin: bool,
    },
    Bootstrap {
        #[arg(long)]
        i_am_sure: bool,
    },
    Migrate {
        #[arg(long)]
        dry_run: bool,

        #[arg(long)]
        init: bool,
    },
}

#[derive(Parser)]
struct Cli {
    #[arg(long)]
    debug: bool,

    #[arg(long)]
    hostname: Host,

    #[command(subcommand)]
    action: Action,
}

fn configure_tracing(level: tracing::Level) -> impl tracing::Subscriber {
    tracing_subscriber::fmt()
        .event_format(tracing_subscriber::fmt::format().compact())
        .with_timer(tracing_subscriber::fmt::time::SystemTime)
        .with_span_events(tracing_subscriber::fmt::format::FmtSpan::NONE)
        .with_ansi(true)
        .with_max_level(level)
        .with_target(true)
        .finish()
}

enum MainResult {
    Success,
    Failure,
    Error(anyhow::Error),
}

#[expect(clippy::print_stderr, reason = "we need to report the error")]
impl process::Termination for MainResult {
    fn report(self) -> process::ExitCode {
        match self {
            Self::Success => process::ExitCode::SUCCESS,
            Self::Failure => process::ExitCode::FAILURE,
            Self::Error(e) => {
                eprintln!("Error: {e}");
                process::ExitCode::FAILURE
            }
        }
    }
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> MainResult {
    async fn inner() -> Result<std::result::Result<(), ()>> {
        let args = Cli::try_parse()?;

        let subscriber = configure_tracing(if args.debug {
            tracing::Level::DEBUG
        } else {
            tracing::Level::INFO
        });

        tracing::subscriber::set_global_default(subscriber)?;

        let running_user = Process::run("id", &["--user", "--name"], &Elevate::No, Stdin::Closed)
            .await?
            .check()?
            .stdout
            .trim()
            .to_owned();

        match args.action {
            #[expect(
                clippy::print_stdout,
                clippy::print_stderr,
                reason = "migrate is more chatty"
            )]
            Action::Migrate { dry_run, init } => {
                let root = match running_user.as_str() {
                    "root" => Elevate::No,
                    _ => Elevate::Sudo(ElevateTo::Root),
                };

                let state = migrations::MachineState::load(&root).await?;

                match state {
                    Some(mut state) => {
                        if init {
                            eprintln!("state already present, --init cannot be done");
                            Ok(Err(()))
                        } else {
                            let mut any_applied = false;
                            for migration in state.migration.unapplied() {
                                if dry_run {
                                    println!("would apply {migration}");
                                } else {
                                    println!("applying {migration}");
                                    migration.apply(&root)?;
                                    state.save_migration(&root, migration).await?;
                                    any_applied = true;
                                }
                            }

                            if any_applied {
                                println!("All migrations applied");
                            }

                            Ok(Ok(()))
                        }
                    }
                    None => {
                        if init {
                            let id = migrations::Migration::latest();
                            let new_state = migrations::MachineState::new(id);
                            if dry_run {
                                println!("would save new state {new_state}");
                            } else {
                                new_state.save(&root).await?;
                            }
                            Ok(Ok(()))
                        } else {
                            Err(anyhow!("no state found, use --init to initialize it"))
                        }
                    }
                }
            }
            Action::Deploy {
                dry_run,
                only_changed,
                clean_packages,
                secrets_stdin,
            } => {
                let root = match running_user.as_str() {
                    "root" => Elevate::No,
                    _ => Elevate::Sudo(ElevateTo::Root),
                };

                let config = args
                    .hostname
                    .machine_config()
                    .with_users()
                    .with_running_user(&running_user);

                let secrets = if secrets_stdin {
                    Some(Secrets::load_from(io::stdin())?)
                } else {
                    None
                };

                let mut plan = Plan::new();

                if let Some(ref secrets) = secrets {
                    plan.set_secrets(SecretsWorkflow::Secrets(secrets));
                } else {
                    plan.set_secrets(SecretsWorkflow::AssumeExisting);
                }

                plan.try_block("root setup", |plan| {
                    root::kernel(plan, &root);
                    root::filesystem(plan, &root);
                    root::hostname(plan, &root, &config);
                    root::locale(plan, &root, &config);
                    root::vconsole(plan, &root);
                    root::fstrim(plan, &root);
                    root::timezone(plan, &root);
                    root::pacman(plan, &root);
                    root::posix(plan, &root);
                    root::networkmanager(plan, &root);
                    root::reflector(plan, &root);
                    root::initrd(plan, &root, &config);
                    root::libvirt(plan, &root);
                    root::docker(plan, &root);
                    root::pipewire(plan, &root);
                    root::fonts(plan, &root);
                    root::smartcard(plan, &root);
                    root::wireshark(plan, &root);
                    root::boot_target(plan, &root, &config);
                    root::timesyncd(plan, &root);
                    root::sudo(plan, &root);
                    root::hardware(plan, &root, &config);
                    root::logind(plan, &root);
                    root::journald(plan, &root);
                    root::wireguard(plan, &root)?;
                    root::sshd(plan, &root)?;
                    root::pkgstats(plan, &root);
                    root::iptables(plan, &root);
                    root::backlight(plan, &root);
                    root::firefox(plan, &root);

                    Ok(())
                })?;

                let mut user_packages: HashMap<
                    MaybeOwned<'_, str>,
                    MaybeOwned<'_, [MaybeOwned<'_, str>]>,
                > = HashMap::new();

                for user in &config.users {
                    plan.try_block_async(Owned(user.config.name.clone()), async |plan| {
                        let user_result =
                            user::configure(plan, &root, user, &config.machine).await?;

                        user_packages.extend(user_result.packages);

                        Ok(())
                    })
                    .await?;
                }

                plan.block("user packages", |plan| {
                    user::packages(plan, &root, &user_packages);
                });

                plan.block("aur packages", |plan| {
                    aur::aur(plan, &root);
                });

                plan.block("application packages", |plan| {
                    for (section, packages) in cfg::packages::APPLICATION_PACKAGES {
                        let packages = packages
                            .iter()
                            .map(|package| Ref(*package))
                            .collect::<Vec<_>>();

                        let path = section.join("/");

                        plan.add(
                            Owned(format!("install packages for {path}")),
                            &root,
                            Packages {
                                install: Owned(packages.clone()),
                                reason: InstallReason::Explicit,
                            },
                        );

                        plan.register_packages(&packages);
                    }
                });

                plan.block("host specials", |plan| {
                    args.hostname.specials(plan, &root);
                });

                let plan_finished = match plan.finish().await? {
                    FinishResult::AssertionsFailed { report } => {
                        report.print();
                        return Ok(Err(()));
                    }
                    FinishResult::Success(plan) => plan,
                };

                let report = plan_finished.report()?;
                report.print(
                    &if only_changed {
                        ReportConfig::OnlyChanged
                    } else {
                        ReportConfig::All
                    },
                    true,
                );

                let has_changes = report.has_changes();

                let plan_approved = plan_finished.approve();

                if !dry_run && has_changes {
                    plan_approved.apply().await?;
                }

                if !dry_run && clean_packages {
                    let managed_packages = plan_approved.managed_packages();
                    remove_unconfigured_packages(&root, dry_run, managed_packages).await?;
                }

                Ok(Ok(()))
            }
            Action::Bootstrap { i_am_sure } => {
                if running_user != "root" {
                    Err(anyhow!("needs to be run as root"))
                } else if i_am_sure {
                    let elevate = Elevate::No;
                    let tempfile = Tempfile::new(&elevate).await?;
                    tempfile.write(args.hostname.bootstrap_script()).await?;

                    Process::run_in_foreground(
                        "bash",
                        &[tempfile.path().as_str()],
                        &elevate,
                        &Stdin::Open,
                    )?
                    .check()?;
                    Ok(Ok(()))
                } else {
                    Err(anyhow!("coward"))
                }
            }
        }
    }

    match inner().await {
        Ok(Ok(())) => MainResult::Success,
        Ok(Err(())) => MainResult::Failure,
        Err(err) => MainResult::Error(err),
    }
}
