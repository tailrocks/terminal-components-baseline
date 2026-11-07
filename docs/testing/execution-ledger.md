# Execution ledger

This ledger is the assignment record for the pre-refactor test programme.
It is not a claim that the cases have already passed.
The architecture refactor is out of scope.

Reference commit: `fffe002e936d3621f22f9132144066a1374d5d74`.
Execution tasks: 180.
Leaf tasks: 170.
Coverage rows: 2223.

Recorded order: G00, G01, G02, G05, G03, G04, then one Showcase route per leaf plus S01.shell, then TablePro, Holla, Jackin, and cross-application leaves, then C01, C02, C03, and C04.

Each leaf has one task id, one case file, one coverage fragment, and one baseline namespace.
Scope cards S01 through S10, T01 through T05, H01 through H06, J01 through J06, and X01 through X03 are not assigned.
Their children are the assigned tasks.

Showcase section inventory: 54 routes, 162 sections, 324 section-size captures.
SpecimenState has 14 states. The catalog lists 13 and omits `editing`.
Option rows execute a default and one boundary, plus `needs` dependencies. They are not a Cartesian product.

## Order

- `G00` (prerequisite, wave A) depends on none.
- `G01` (prerequisite, wave A) depends on G00.
- `G02` (prerequisite, wave B) depends on G01.
- `G05` (prerequisite, wave B) depends on G00, G01, G02.
- `G03` (prerequisite, wave B) depends on G05.
- `G04` (prerequisite, wave B) depends on G01, G03.
- `S01.shell` (leaf, wave C) depends on G03, G04, G05.
- `S01.overview` (leaf, wave C) depends on G03, G04, G05.
- `S01.chrome` (leaf, wave C) depends on G03, G04, G05.
- `S01.statushints` (leaf, wave C) depends on G03, G04, G05.
- `S01.breadcrumbsrails` (leaf, wave C) depends on G03, G04, G05.
- `S01.helpkeys` (leaf, wave C) depends on G03, G04, G05.
- `S01.quitleave` (leaf, wave C) depends on G03, G04, G05.
- `S01.focusnavigation` (leaf, wave C) depends on G03, G04, G05.
- `S02.buttons` (leaf, wave C) depends on G03, G04, G05.
- `S02.inputs` (leaf, wave C) depends on G03, G04, G05.
- `S02.textareas` (leaf, wave C) depends on G03, G04, G05.
- `S02.chipsselects` (leaf, wave C) depends on G03, G04, G05.
- `S02.forms` (leaf, wave C) depends on G03, G04, G05.
- `S02.checksradios` (leaf, wave C) depends on G03, G04, G05.
- `S02.toolbars` (leaf, wave C) depends on G03, G04, G05.
- `S03.basictable` (leaf, wave C) depends on G03, G04, G05.
- `S03.editable` (leaf, wave C) depends on G03, G04, G05.
- `S03.datagrid` (leaf, wave C) depends on G03, G04, G05.
- `S03.selection` (leaf, wave C) depends on G03, G04, G05.
- `S03.celleditors` (leaf, wave C) depends on G03, G04, G05.
- `S03.columns` (leaf, wave C) depends on G03, G04, G05.
- `S04.lists` (leaf, wave C) depends on G03, G04, G05.
- `S04.trees` (leaf, wave C) depends on G03, G04, G05.
- `S04.sidebars` (leaf, wave C) depends on G03, G04, G05.
- `S04.tabs` (leaf, wave C) depends on G03, G04, G05.
- `S04.menus` (leaf, wave C) depends on G03, G04, G05.
- `S05.dialogs` (leaf, wave C) depends on G03, G04, G05.
- `S05.pickers` (leaf, wave C) depends on G03, G04, G05.
- `S05.dialogkinds` (leaf, wave C) depends on G03, G04, G05.
- `S05.popupstooltips` (leaf, wave C) depends on G03, G04, G05.
- `S05.filebrowser` (leaf, wave C) depends on G03, G04, G05.
- `S06.progress` (leaf, wave C) depends on G03, G04, G05.
- `S06.emptyerror` (leaf, wave C) depends on G03, G04, G05.
- `S06.meters` (leaf, wave C) depends on G03, G04, G05.
- `S06.notices` (leaf, wave C) depends on G03, G04, G05.
- `S07.panels` (leaf, wave C) depends on G03, G04, G05.
- `S07.scrolling` (leaf, wave C) depends on G03, G04, G05.
- `S07.splitsstacks` (leaf, wave C) depends on G03, G04, G05.
- `S07.masterdetail` (leaf, wave C) depends on G03, G04, G05.
- `S07.panetree` (leaf, wave C) depends on G03, G04, G05.
- `S08.codeeditor` (leaf, wave C) depends on G03, G04, G05.
- `S08.diff` (leaf, wave C) depends on G03, G04, G05.
- `S08.terminal` (leaf, wave C) depends on G03, G04, G05.
- `S08.textutilities` (leaf, wave C) depends on G03, G04, G05.
- `S09.settings` (leaf, wave C) depends on G03, G04, G05.
- `S09.taskrunner` (leaf, wave C) depends on G03, G04, G05.
- `S09.queryworkbench` (leaf, wave C) depends on G03, G04, G05.
- `S09.connections` (leaf, wave C) depends on G03, G04, G05.
- `S09.finder` (leaf, wave C) depends on G03, G04, G05.
- `S10.diskcleanup` (leaf, wave C) depends on G03, G04, G05.
- `S10.activity` (leaf, wave C) depends on G03, G04, G05.
- `S10.workspaces` (leaf, wave C) depends on G03, G04, G05.
- `S10.accountsusage` (leaf, wave C) depends on G03, G04, G05.
- `S10.launchcockpit` (leaf, wave C) depends on G03, G04, G05.
- `S10.readonlyreport` (leaf, wave C) depends on G03, G04, G05.
- `T01.connections` (leaf, wave C) depends on G03, G04, G05.
- `T01.basic` (leaf, wave C) depends on G03, G04, G05.
- `T01.advanced` (leaf, wave C) depends on G03, G04, G05.
- `T01.test` (leaf, wave C) depends on G03, G04, G05.
- `T01.leave` (leaf, wave C) depends on G03, G04, G05.
- `T02.explorer` (leaf, wave C) depends on G03, G04, G05.
- `T02.structure` (leaf, wave C) depends on G03, G04, G05.
- `T02.data` (leaf, wave C) depends on G03, G04, G05.
- `T02.editors` (leaf, wave C) depends on G03, G04, G05.
- `T02.filters` (leaf, wave C) depends on G03, G04, G05.
- `T02.commit` (leaf, wave C) depends on G03, G04, G05.
- `T02.discard` (leaf, wave C) depends on G03, G04, G05.
- `T02.viewers` (leaf, wave C) depends on G03, G04, G05.
- `T03.query` (leaf, wave C) depends on G03, G04, G05.
- `T03.completion` (leaf, wave C) depends on G03, G04, G05.
- `T03.running` (leaf, wave C) depends on G03, G04, G05.
- `T03.confirm` (leaf, wave C) depends on G03, G04, G05.
- `T03.rows` (leaf, wave C) depends on G03, G04, G05.
- `T03.affected` (leaf, wave C) depends on G03, G04, G05.
- `T03.error` (leaf, wave C) depends on G03, G04, G05.
- `T03.plan` (leaf, wave C) depends on G03, G04, G05.
- `T03.cancelled` (leaf, wave C) depends on G03, G04, G05.
- `T04.history` (leaf, wave C) depends on G03, G04, G05.
- `T04.quickly` (leaf, wave C) depends on G03, G04, G05.
- `T04.tabs` (leaf, wave C) depends on G03, G04, G05.
- `T04.preview` (leaf, wave C) depends on G03, G04, G05.
- `T04.close` (leaf, wave C) depends on G03, G04, G05.
- `T05.menus` (leaf, wave C) depends on G03, G04, G05.
- `T05.help` (leaf, wave C) depends on G03, G04, G05.
- `T05.context` (leaf, wave C) depends on G03, G04, G05.
- `T05.viewers` (leaf, wave C) depends on G03, G04, G05.
- `T05.maximize` (leaf, wave C) depends on G03, G04, G05.
- `T05.disconnect` (leaf, wave C) depends on G03, G04, G05.
- `T05.quit` (leaf, wave C) depends on G03, G04, G05.
- `H01.finder` (leaf, wave C) depends on G03, G04, G05.
- `H01.scope` (leaf, wave C) depends on G03, G04, G05.
- `H01.alternatives` (leaf, wave C) depends on G03, G04, G05.
- `H01.why` (leaf, wave C) depends on G03, G04, G05.
- `H01.memory` (leaf, wave C) depends on G03, G04, G05.
- `H02.trust` (leaf, wave C) depends on G03, G04, G05.
- `H02.args` (leaf, wave C) depends on G03, G04, G05.
- `H02.config` (leaf, wave C) depends on G03, G04, G05.
- `H02.reload` (leaf, wave C) depends on G03, G04, G05.
- `H02.validation` (leaf, wave C) depends on G03, G04, G05.
- `H03.output` (leaf, wave C) depends on G03, G04, G05.
- `H03.input` (leaf, wave C) depends on G03, G04, G05.
- `H03.picker` (leaf, wave C) depends on G03, G04, G05.
- `H03.attach` (leaf, wave C) depends on G03, G04, G05.
- `H03.guards` (leaf, wave C) depends on G03, G04, G05.
- `H04.actions` (leaf, wave C) depends on G03, G04, G05.
- `H04.snapshots-git` (leaf, wave C) depends on G03, G04, G05.
- `H04.snapshots-host` (leaf, wave C) depends on G03, G04, G05.
- `H04.snapshots-docker` (leaf, wave C) depends on G03, G04, G05.
- `H04.snapshots-services` (leaf, wave C) depends on G03, G04, G05.
- `H04.plan` (leaf, wave C) depends on G03, G04, G05.
- `H05.files` (leaf, wave C) depends on G03, G04, G05.
- `H05.find` (leaf, wave C) depends on G03, G04, G05.
- `H05.previews` (leaf, wave C) depends on G03, G04, G05.
- `H05.scan` (leaf, wave C) depends on G03, G04, G05.
- `H05.disk` (leaf, wave C) depends on G03, G04, G05.
- `H06.cleanup` (leaf, wave C) depends on G03, G04, G05.
- `H06.gate` (leaf, wave C) depends on G03, G04, G05.
- `H06.report` (leaf, wave C) depends on G03, G04, G05.
- `H06.log` (leaf, wave C) depends on G03, G04, G05.
- `H06.quit` (leaf, wave C) depends on G03, G04, G05.
- `J01.status` (leaf, wave C) depends on G03, G04, G05.
- `J01.refresh` (leaf, wave C) depends on G03, G04, G05.
- `J01.stop` (leaf, wave C) depends on G03, G04, G05.
- `J01.purge` (leaf, wave C) depends on G03, G04, G05.
- `J01.delete` (leaf, wave C) depends on G03, G04, G05.
- `J01.prewarm` (leaf, wave C) depends on G03, G04, G05.
- `J01.prelude` (leaf, wave C) depends on G03, G04, G05, J02.save.
- `J02.general` (leaf, wave C) depends on G03, G04, G05.
- `J02.mounts` (leaf, wave C) depends on G03, G04, G05.
- `J02.roles` (leaf, wave C) depends on G03, G04, G05.
- `J02.environments` (leaf, wave C) depends on G03, G04, G05.
- `J02.accounts` (leaf, wave C) depends on G03, G04, G05.
- `J02.save` (leaf, wave C) depends on G03, G04, G05.
- `J03.general` (leaf, wave C) depends on G03, G04, G05.
- `J03.mounts` (leaf, wave C) depends on G03, G04, G05, J02.mounts.
- `J03.environments` (leaf, wave C) depends on G03, G04, G05, J02.environments.
- `J03.agents` (leaf, wave C) depends on G03, G04, G05.
- `J03.trust` (leaf, wave C) depends on G03, G04, G05.
- `J03.leave` (leaf, wave C) depends on G03, G04, G05.
- `J04.accounts` (leaf, wave C) depends on G03, G04, G05.
- `J04.form` (leaf, wave C) depends on G03, G04, G05.
- `J04.chooser` (leaf, wave C) depends on G03, G04, G05.
- `J04.usage` (leaf, wave C) depends on G03, G04, G05.
- `J05.intro` (leaf, wave C) depends on G03, G04, G05.
- `J05.launch` (leaf, wave C) depends on G03, G04, G05.
- `J05.logs` (leaf, wave C) depends on G03, G04, G05.
- `J05.failure` (leaf, wave C) depends on G03, G04, G05.
- `J05.handoff` (leaf, wave C) depends on G03, G04, G05.
- `J05.outro` (leaf, wave C) depends on G03, G04, G05.
- `J06.panes` (leaf, wave C) depends on G03, G04, G05.
- `J06.tabs` (leaf, wave C) depends on G03, G04, G05.
- `J06.prefix` (leaf, wave C) depends on G03, G04, G05.
- `J06.reconnect` (leaf, wave C) depends on G03, G04, G05.
- `J06.takeover` (leaf, wave C) depends on G03, G04, G05.
- `J06.inspect` (leaf, wave C) depends on G03, G04, G05.
- `J06.exit` (leaf, wave C) depends on G03, G04, G05.
- `J06.export` (leaf, wave C) depends on G03, G04, G05.
- `X01.cli` (leaf, wave D) depends on G03, G04, G05.
- `X01.env` (leaf, wave D) depends on G03, G04, G05.
- `X01.input` (leaf, wave D) depends on G03, G04, G05.
- `X01.clipboard` (leaf, wave D) depends on G03, G04, G05.
- `X02.startup` (leaf, wave D) depends on G03, G04, G05.
- `X02.resize` (leaf, wave D) depends on G03, G04, G05, T05.quit, H03.guards, J06.exit.
- `X02.suspend` (leaf, wave D) depends on G03, G04, G05.
- `X02.exit` (leaf, wave D) depends on G03, G04, G05, T05.quit, H03.guards, J05.failure, J06.exit.
- `X03.color` (leaf, wave D) depends on G03, G04, G05, S02.buttons, S01.shell, T05.menus, H01.finder, J01.status.
- `X03.focus` (leaf, wave D) depends on G03, G04, G05, S04.lists, S05.dialogs.
- `X03.chrome` (leaf, wave D) depends on G03, G04, G05, S07.scrolling, S07.splitsstacks.
- `X03.glyphs` (leaf, wave D) depends on G03, G04, G05, S02.buttons.
- `X03.motion` (leaf, wave D) depends on G03, G04, G05, J05.intro, J05.handoff, J05.outro, S06.progress.
- `C01` (closing, wave E) depends on G04, G05, S01.shell, S01.overview, S01.chrome, S01.statushints, S01.breadcrumbsrails, S01.helpkeys, S01.quitleave, S01.focusnavigation, S02.buttons, S02.inputs, S02.textareas, S02.chipsselects, S02.forms, S02.checksradios, S02.toolbars, S03.basictable, S03.editable, S03.datagrid, S03.selection, S03.celleditors, S03.columns, S04.lists, S04.trees, S04.sidebars, S04.tabs, S04.menus, S05.dialogs, S05.pickers, S05.dialogkinds, S05.popupstooltips, S05.filebrowser, S06.progress, S06.emptyerror, S06.meters, S06.notices, S07.panels, S07.scrolling, S07.splitsstacks, S07.masterdetail, S07.panetree, S08.codeeditor, S08.diff, S08.terminal, S08.textutilities, S09.settings, S09.taskrunner, S09.queryworkbench, S09.connections, S09.finder, S10.diskcleanup, S10.activity, S10.workspaces, S10.accountsusage, S10.launchcockpit, S10.readonlyreport, T01.connections, T01.basic, T01.advanced, T01.test, T01.leave, T02.explorer, T02.structure, T02.data, T02.editors, T02.filters, T02.commit, T02.discard, T02.viewers, T03.query, T03.completion, T03.running, T03.confirm, T03.rows, T03.affected, T03.error, T03.plan, T03.cancelled, T04.history, T04.quickly, T04.tabs, T04.preview, T04.close, T05.menus, T05.help, T05.context, T05.viewers, T05.maximize, T05.disconnect, T05.quit, H01.finder, H01.scope, H01.alternatives, H01.why, H01.memory, H02.trust, H02.args, H02.config, H02.reload, H02.validation, H03.output, H03.input, H03.picker, H03.attach, H03.guards, H04.actions, H04.snapshots-git, H04.snapshots-host, H04.snapshots-docker, H04.snapshots-services, H04.plan, H05.files, H05.find, H05.previews, H05.scan, H05.disk, H06.cleanup, H06.gate, H06.report, H06.log, H06.quit, J01.status, J01.refresh, J01.stop, J01.purge, J01.delete, J01.prewarm, J01.prelude, J02.general, J02.mounts, J02.roles, J02.environments, J02.accounts, J02.save, J03.general, J03.mounts, J03.environments, J03.agents, J03.trust, J03.leave, J04.accounts, J04.form, J04.chooser, J04.usage, J05.intro, J05.launch, J05.logs, J05.failure, J05.handoff, J05.outro, J06.panes, J06.tabs, J06.prefix, J06.reconnect, J06.takeover, J06.inspect, J06.exit, J06.export, X01.cli, X01.env, X01.input, X01.clipboard, X02.startup, X02.resize, X02.suspend, X02.exit, X03.color, X03.focus, X03.chrome, X03.glyphs, X03.motion.
- `C02` (closing, wave F) depends on C01.
- `C03` (closing, wave F) depends on C02.
- `C04` (closing, wave F) depends on C03.

## Current evidence

The Nextest list at the ledger draft contains 93 functional tests.
The baseline tree contains 105 samples.
Those tests are classified in `refactor-coverage.json` and are not counted as PASS for the new rows.

## Acceptance rule

A row stays BLOCKED until a PTY case drives the real binary, asserts the semantic result and the forbidden effect, and compares a reference capture.
NOT_APPLICABLE requires source evidence.
An implemented state without a test cannot be closed as NOT_APPLICABLE.

