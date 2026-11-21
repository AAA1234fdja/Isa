using System.Diagnostics;

namespace Isa.Desktop;

// These restrictions reduce accidental exposure; they are not authentication
// or a security boundary against a malicious process running as the same user.
internal static class DesktopPolicy
{
    internal const string Address = "127.0.0.1:4175";
    internal const string StartMessage = "ISA preview listening on http://" + Address;
    internal static readonly Uri Home = new("http://" + Address + "/");

    internal static void ConfigureBackend(ProcessStartInfo info)
    {
        // Forward only explicit configuration for the supervised experimental
        // features. Arbitrary inherited ISA settings remain blocked.
        info.Environment.TryGetValue("ISA_WEB_EXEC_WORKSPACE", out string? workspace);
        info.Environment.TryGetValue("ISA_WEB_EXEC_TOKEN", out string? token);
        info.Environment.TryGetValue("ISA_WEB_EXEC_PROFILE", out string? profile);
        info.Environment.TryGetValue("ISA_WEB_AGENT_WORKSPACE", out string? agentWorkspace);
        info.Environment.TryGetValue("ISA_WEB_MODEL", out string? model);
        foreach (string key in info.Environment.Keys.ToArray())
            if (key.StartsWith("ISA_", StringComparison.OrdinalIgnoreCase))
                info.Environment.Remove(key);
        info.Environment["ISA_ADDRESS"] = Address;
        info.Environment["ISA_ASSET_MODE"] = "embedded";
        // These switches enable only already-connected Windows implementations;
        // workspace, token, per-chat policy and user approvals remain required.
        info.Environment["ISA_WEB_WINDOWS_READ_ONLY"] = "1";
        info.Environment["ISA_WEB_WINDOWS_SHELL_EXPERIMENTAL"] = "1";
        info.Environment["ISA_WEB_AUDIT_ENCRYPTED"] = "1";
        info.Environment["ISA_WEB_WINDOWS_AUDIT_EXPERIMENTAL"] = "1";
        if (IsWorkspace(workspace) && IsToken(token))
        {
            info.Environment["ISA_WEB_EXEC_WORKSPACE"] = workspace!;
            info.Environment["ISA_WEB_EXEC_TOKEN"] = token!;
            // Never discard an explicit sandbox request and silently run on
            // the host. Unsupported or malformed values fail closed in Rust.
            if (profile is not null)
                info.Environment["ISA_WEB_EXEC_PROFILE"] = profile.Length <= 256 ? profile : "invalid";
        }
        if (IsWorkspace(agentWorkspace) && !string.IsNullOrWhiteSpace(model) && model.Length <= 256 && !model.Any(char.IsControl))
        {
            info.Environment["ISA_WEB_AGENT_WORKSPACE"] = agentWorkspace!;
            info.Environment["ISA_WEB_MODEL"] = model;
        }
    }

    private static bool IsWorkspace(string? value) =>
        !string.IsNullOrWhiteSpace(value) && Path.IsPathFullyQualified(value) && Directory.Exists(value);

    private static bool IsToken(string? value) =>
        value is { Length: >= 32 and <= 256 } && value.All(c => c is >= '!' and <= '~');

    internal static bool IsAllowedNavigation(string? target)
    {
        return Uri.TryCreate(target, UriKind.Absolute, out var uri)
            && uri.Scheme == Uri.UriSchemeHttp
            && uri.Host == Home.Host
            && uri.Port == Home.Port
            && string.IsNullOrEmpty(uri.UserInfo);
    }

    internal static bool IsLocalBlobDownload(string? target)
    {
        return target is not null && target.StartsWith("blob:", StringComparison.Ordinal)
            && IsAllowedNavigation(target[5..]);
    }

    internal static bool TryExternalHttps(string? target, out Uri? uri)
    {
        uri = null;
        if (string.IsNullOrEmpty(target) || target.Length > 2048 ||
            target.Any(c => char.IsControl(c) || char.IsWhiteSpace(c) ||
                c is '\u061c' or '\u200e' or '\u200f' or >= '\u202a' and <= '\u202e' or >= '\u2066' and <= '\u2069'))
            return false;
        if (!Uri.TryCreate(target, UriKind.Absolute, out var parsed) ||
            parsed.Scheme != Uri.UriSchemeHttps || string.IsNullOrEmpty(parsed.Host) ||
            !string.IsNullOrEmpty(parsed.UserInfo))
            return false;
        uri = parsed;
        return true;
    }
}
