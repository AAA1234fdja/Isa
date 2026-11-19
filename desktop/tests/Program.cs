using System.Diagnostics;
using Isa.Desktop;

static void Check(bool condition, string message)
{
    if (!condition) throw new InvalidOperationException(message);
}

foreach (string target in new[] {
    "http://127.0.0.1:4175/",
    "http://127.0.0.1:4175/index.html#chat",
    "http://127.0.0.1:4175/assets/layout.js",
}) Check(DesktopPolicy.IsAllowedNavigation(target), $"Expected allowed: {target}");

foreach (string target in new[] {
    "http://localhost:4175/",
    "https://127.0.0.1:4175/",
    "http://127.0.0.1:4174/",
    "http://127.0.0.1:4175.evil.test/",
    "http://user@127.0.0.1:4175/",
    "http://example.com/",
    "javascript:alert(1)",
    "file:///C:/test.html",
    "about:blank",
    "/relative",
    "",
}) Check(!DesktopPolicy.IsAllowedNavigation(target), $"Expected blocked: {target}");
Check(!DesktopPolicy.IsAllowedNavigation(null), "Null must be blocked");

var info = new ProcessStartInfo("unused");
info.Environment["ISA_WEB_EXEC_TOKEN"] = "test-value";
info.Environment["ISA_WEB_WINDOWS_SHELL_EXPERIMENTAL"] = "1";
info.Environment["isa_test"] = "test-value";
info.Environment["NOT_ISA_TEST"] = "preserved";
DesktopPolicy.ConfigureBackend(info);
Check(!info.Environment.ContainsKey("ISA_WEB_EXEC_TOKEN"), "Token was inherited");
Check(!info.Environment.ContainsKey("ISA_WEB_WINDOWS_SHELL_EXPERIMENTAL"), "Shell was enabled");
Check(!info.Environment.ContainsKey("isa_test"), "Case-insensitive prefix was not removed");
Check(info.Environment["NOT_ISA_TEST"] == "preserved", "Unrelated environment was changed");
Check(info.Environment["ISA_ADDRESS"] == DesktopPolicy.Address, "Wrong address");
Check(info.Environment["ISA_ASSET_MODE"] == "embedded", "Wrong asset mode");
Console.WriteLine("Desktop prototype policy checks passed.");
