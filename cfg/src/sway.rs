#[derive(Copy, Clone)]
pub(crate) enum WorkspaceApp {
    Firefox,
    Terminal,
    Steam,
    Discord,
    Spotify,
    Game,
    Pim,
    Keepass,
    Meet,
    Tv,
}

impl WorkspaceApp {
    pub(crate) fn icon(self) -> &'static str {
        match self {
            Self::Firefox => "",
            Self::Terminal => "",
            Self::Steam => "",
            Self::Discord => " ",
            Self::Spotify => "",
            Self::Game => "",
            Self::Pim => "",
            Self::Keepass => "",
            Self::Meet => "",
            Self::Tv => "",
        }
    }
}

pub struct WorkspaceConfig {
    pub(crate) num1: Option<WorkspaceApp>,
    pub(crate) num2: Option<WorkspaceApp>,
    pub(crate) num3: Option<WorkspaceApp>,
    pub(crate) num4: Option<WorkspaceApp>,
    pub(crate) num5: Option<WorkspaceApp>,
    pub(crate) num6: Option<WorkspaceApp>,
    pub(crate) num7: Option<WorkspaceApp>,
    pub(crate) num8: Option<WorkspaceApp>,
    pub(crate) num9: Option<WorkspaceApp>,
    pub(crate) num0: Option<WorkspaceApp>,
}

macro_rules! workspace_config_sway_workspace_name {
    ($self:ident, $v:ident, $i:literal, $s:literal) => {
        paste::paste! {
            let name = $self.[<num$i>]
                .map(|app| format!(concat!($s, ": {}"), app.icon()))
                .unwrap_or_else(|| $i.to_owned());

            let mut out = vec![];

            out.push(
                format!("bindsym $mod+{i} workspace \"{name}\"", i = $i)
            );

            out.push(
                format!("bindsym $mod+Shift+{i} move container to workspace \"{name}\"", i = $i)
            );

            if let Some(app) = $self.[<num$i>] {
                match app {
                    WorkspaceApp::Steam => {
                        out.push(
                             format!("assign [class=\"^[Ss]team$\"] \"{name}\"")
                        )
                    },
                    WorkspaceApp::Discord => {
                        out.push(
                             format!("assign [app_id=\"^discord$\"] \"{name}\"")
                        )
                    },
                    WorkspaceApp::Spotify => {
                        out.push(
                             format!("assign [app_id=\"^spotify$\"] \"{name}\"")
                        )
                    },
                    WorkspaceApp::Keepass => {
                        out.push(
                             format!("assign [app_id=\"^org.keepassxc.KeePassXC$\"] \"{name}\"")
                        )
                    },
                    WorkspaceApp::Firefox => (),
                    WorkspaceApp::Terminal => (),
                    WorkspaceApp::Game => (),
                    WorkspaceApp::Pim => (),
                    WorkspaceApp::Meet => (),
                    WorkspaceApp::Tv => (),
                }
            }

            $v.push(out.join("\n"));
        }
    };
}

impl WorkspaceConfig {
    pub fn to_sway_config(&self) -> String {
        let mut out = vec![];

        workspace_config_sway_workspace_name!(self, out, "1", "1");
        workspace_config_sway_workspace_name!(self, out, "2", "2");
        workspace_config_sway_workspace_name!(self, out, "3", "3");
        workspace_config_sway_workspace_name!(self, out, "4", "4");
        workspace_config_sway_workspace_name!(self, out, "5", "5");
        workspace_config_sway_workspace_name!(self, out, "6", "6");
        workspace_config_sway_workspace_name!(self, out, "7", "7");
        workspace_config_sway_workspace_name!(self, out, "8", "8");
        workspace_config_sway_workspace_name!(self, out, "9", "9");
        workspace_config_sway_workspace_name!(self, out, "0", "10");

        out.join("\n\n")
    }
}
