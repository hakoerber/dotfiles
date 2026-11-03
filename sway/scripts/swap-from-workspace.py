import json
import subprocess
import sys


def bail(msg):
    print(msg)
    sys.exit(1)


workspace_to_swap_with = int(sys.argv[1])

cmds = []

workspaces = json.loads(
    subprocess.run(
        ["swaymsg", "--raw", "--type", "get_workspaces"],
        stdout=subprocess.PIPE,
        check=True,
        text=True,
    ).stdout
)

try:
    target_workspace = next(
        filter(lambda w: int(w["num"]) == workspace_to_swap_with, workspaces)
    )
except StopIteration:
    bail(f"target workspace {workspace_to_swap_with} not found")

try:
    current_workspace = next(filter(lambda w: w["focused"], workspaces))
except StopIteration:
    bail("current workspace not found")

if current_workspace["num"] == target_workspace["num"]:
    bail("you are already on the target workspace")

# we need to remember the workspace that is currently active on the output that
# currently has the target workspace, so we can switch back to it later
target_output = target_workspace["output"]

try:
    active_workspace_on_target_output = next(
        filter(lambda w: w["output"] == target_output and w["visible"], workspaces)
    )
except StopIteration:
    bail("could not detect active workspace on target output")

# remove potential leftover marks
cmds.append("unmark _destination")
cmds.append("unmark _source")

# mark source (remove potential existing marks)
cmds.append("mark --add _source")

# mark source (remove potential existing marks)
cmds.append(f"workspace number {target_workspace['num']}")
cmds.append("mark --add _destination")

# do the swap
cmds.append('[con_mark="^_destination$"] swap container with mark _source')

# remove marks on destination first, so the focus and selected workspace can
# be done afterwards
cmds.append('[con_mark="^_destination$"] focus')
cmds.append("unmark _destination")

# switch back to workspace that was originally active on target output
if active_workspace_on_target_output["output"] != target_workspace["output"]:
    cmds.append(f"workspace number {active_workspace_on_target_output['num']}")

# remove mark on source, so focus stays there
cmds.append('[con_mark="^_source$"] focus')
cmds.append("unmark _source")

# switch back to original workspace
cmds.append(f"workspace number {current_workspace['num']}")

subprocess.run(["swaymsg", ";".join(cmds)], check=True)
