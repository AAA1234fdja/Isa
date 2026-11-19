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
}
