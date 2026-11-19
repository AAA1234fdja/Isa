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
        foreach (string key in info.Environment.Keys.ToArray())
            if (key.StartsWith("ISA_", StringComparison.OrdinalIgnoreCase))
                info.Environment.Remove(key);
        info.Environment["ISA_ADDRESS"] = Address;
        info.Environment["ISA_ASSET_MODE"] = "embedded";
    }

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
