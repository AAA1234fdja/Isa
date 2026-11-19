using System.Windows.Forms;

namespace Isa.Desktop;

internal static class Program
{
    [STAThread]
    private static int Main(string[] args)
    {
        bool smokeTest = args.Length == 1 && args[0] == "--smoke-test";
        if (args.Length != 0 && !smokeTest) return 2;
        ApplicationConfiguration.Initialize();
        using var window = new MainWindow(smokeTest);
        Application.Run(window);
        return window.ExitCode;
    }
}
