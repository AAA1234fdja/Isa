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

Check(DesktopPolicy.IsLocalBlobDownload("blob:http://127.0.0.1:4175/7ea467bf-817a-4653-97a5-76c6d542b6e1"), "Local blob should be eligible for reviewed export");
foreach (string target in new[] {
    "https://example.com/file.json",
    "data:text/plain,hello",
    "blob:https://example.com/file",
    "blob:http://127.0.0.1:4174/file",
    "blob:http://127.0.0.1:4175@evil.example/file",
    "blob:file:///C:/secrets",
    "",
}) Check(!DesktopPolicy.IsLocalBlobDownload(target), $"Expected download blocked: {target}");
Check(!DesktopPolicy.IsLocalBlobDownload(null), "Null download must be blocked");

foreach (string target in new[] { "https://example.com/path", "https://example.com:8443/" })
    Check(DesktopPolicy.TryExternalHttps(target, out _), $"Expected eligible HTTPS link: {target}");
foreach (string target in new[] {
    "http://example.com", "javascript:alert(1)", "file:///C:/secret", "https://user:password@example.com/",
    "https://example.com/with space", "https://example.com/\u202Eevil", "", "/relative"
}) Check(!DesktopPolicy.TryExternalHttps(target, out _), $"Expected external link blocked: {target}");
Check(!DesktopPolicy.TryExternalHttps(null, out _), "Null external link must be blocked");
Check(!DesktopPolicy.TryExternalHttps("https://example.com/" + new string('x', 2048), out _), "Oversized external link must be blocked");

var info = new ProcessStartInfo("unused");
info.Environment["ISA_WEB_EXEC_TOKEN"] = "test-value";
info.Environment["ISA_WEB_WINDOWS_SHELL_EXPERIMENTAL"] = "1";
info.Environment["isa_test"] = "test-value";
info.Environment["NOT_ISA_TEST"] = "preserved";
DesktopPolicy.ConfigureBackend(info);
Check(!info.Environment.ContainsKey("ISA_WEB_EXEC_TOKEN"), "Token was inherited");
Check(info.Environment["ISA_WEB_WINDOWS_SHELL_EXPERIMENTAL"] == "1", "Experimental shell switch missing");
Check(info.Environment["ISA_WEB_WINDOWS_READ_ONLY"] == "1", "Experimental read-only switch missing");
Check(info.Environment["ISA_WEB_AUDIT_ENCRYPTED"] == "1", "Encrypted audit switch missing");
Check(info.Environment["ISA_WEB_WINDOWS_AUDIT_EXPERIMENTAL"] == "1", "Experimental audit switch missing");
Check(!info.Environment.ContainsKey("isa_test"), "Case-insensitive prefix was not removed");
Check(info.Environment["NOT_ISA_TEST"] == "preserved", "Unrelated environment was changed");
Check(info.Environment["ISA_ADDRESS"] == DesktopPolicy.Address, "Wrong address");
Check(info.Environment["ISA_ASSET_MODE"] == "embedded", "Wrong asset mode");
var configured = new ProcessStartInfo("unused");
configured.Environment["ISA_WEB_EXEC_WORKSPACE"] = Path.GetTempPath();
configured.Environment["ISA_WEB_EXEC_TOKEN"] = new string('a', 64);
configured.Environment["ISA_WEB_EXEC_PROFILE"] = "sandboxed";
configured.Environment["ISA_WEB_AGENT_WORKSPACE"] = Path.GetTempPath();
configured.Environment["ISA_WEB_MODEL"] = "test-model";
configured.Environment["ISA_WEB_MCP_COMMAND"] = "must-not-be-forwarded";
DesktopPolicy.ConfigureBackend(configured);
Check(configured.Environment["ISA_WEB_EXEC_WORKSPACE"] == Path.GetTempPath(), "Workspace not forwarded");
Check(configured.Environment["ISA_WEB_EXEC_TOKEN"] == new string('a', 64), "Valid token not forwarded");
Check(configured.Environment["ISA_WEB_EXEC_PROFILE"] == "sandboxed", "Sandbox request was silently dropped");
Check(configured.Environment["ISA_WEB_AGENT_WORKSPACE"] == Path.GetTempPath(), "Agent workspace not forwarded");
Check(configured.Environment["ISA_WEB_MODEL"] == "test-model", "Model not forwarded");
Check(!configured.Environment.ContainsKey("ISA_WEB_MCP_COMMAND"), "MCP command must not be inherited");
var invalid = new ProcessStartInfo("unused");
invalid.Environment["ISA_WEB_EXEC_WORKSPACE"] = Path.GetTempPath();
invalid.Environment["ISA_WEB_EXEC_TOKEN"] = "short";
invalid.Environment["ISA_WEB_AGENT_WORKSPACE"] = "relative-path";
invalid.Environment["ISA_WEB_MODEL"] = "model";
DesktopPolicy.ConfigureBackend(invalid);
Check(!invalid.Environment.ContainsKey("ISA_WEB_EXEC_WORKSPACE"), "Incomplete action configuration must be removed");
Check(!invalid.Environment.ContainsKey("ISA_WEB_EXEC_TOKEN"), "Invalid token must be removed");
Check(!invalid.Environment.ContainsKey("ISA_WEB_AGENT_WORKSPACE"), "Invalid agent workspace must be removed");
Check(!invalid.Environment.ContainsKey("ISA_WEB_MODEL"), "Incomplete model configuration must be removed");
Console.WriteLine("Desktop prototype policy checks passed.");
