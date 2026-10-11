// openconnect's --script on Windows. openconnect runs scripts through
// cscript.exe (and waits at most 10 s), so this starts crake-hook.exe from
// the same folder, hidden, with openconnect's environment, and passes on
// its exit code.
var shell = new ActiveXObject("WScript.Shell");
var dir = WScript.ScriptFullName.replace(/[^\\]+$/, "");
WScript.Quit(shell.Run('"' + dir + 'crake-hook.exe"', 0, true));
