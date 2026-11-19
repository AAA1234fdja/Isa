using System.Diagnostics;
using Microsoft.Web.WebView2.Core;
using Microsoft.Web.WebView2.WinForms;

namespace Isa.Desktop;

// Experimental desktop wrapper. This is not an installer or a security sandbox.
internal sealed class MainWindow : Form
{
    private readonly WebView2 webView = new() { Dock = DockStyle.Fill };
    private readonly CancellationTokenSource closing = new();
    private readonly bool smokeTest;
    private readonly System.Windows.Forms.Timer? smokeTimer;
    private readonly string? smokeProfile;
    private Process? backend;
    private volatile bool backendReady;
    private volatile bool stopping;
    private bool reportingFailure;
    internal int ExitCode { get; private set; }

    public MainWindow(bool smokeTest)
    {
        this.smokeTest = smokeTest;
        ExitCode = smokeTest ? 1 : 0;
        if (smokeTest)
        {
            // Keep CI smoke tests out of the user's persistent WebView profile.
            smokeProfile = Path.Combine(Path.GetTempPath(), "isa-desktop-smoke-" + Guid.NewGuid().ToString("N"));
            smokeTimer = new System.Windows.Forms.Timer { Interval = 45000 };
            smokeTimer.Tick += (_, _) => Fail("Desktop smoke test timed out.");
            smokeTimer.Start();
        }
        Text = "ISA — Desktop prototype";
        Width = 1280;
        Height = 820;
        MinimumSize = new System.Drawing.Size(800, 600);
        Controls.Add(webView);
        Shown += OnShown;
        FormClosing += OnClosing;
    }

    private async void OnShown(object? sender, EventArgs e)
    {
        try
        {
            await StartBackendAsync(closing.Token);
            var profile = smokeProfile ?? Path.Combine(
                Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
                "ISA", "DesktopPrototype", "WebView2");
            var environment = await CoreWebView2Environment.CreateAsync(userDataFolder: profile);
            await webView.EnsureCoreWebView2Async(environment);
            if (closing.IsCancellationRequested) return;

            var core = webView.CoreWebView2;
            core.Settings.AreHostObjectsAllowed = false;
            core.Settings.IsWebMessageEnabled = false;
            core.NavigationStarting += RestrictNavigation;
            core.FrameNavigationStarting += RestrictNavigation;
            core.NewWindowRequested += (_, args) => args.Handled = true;
            core.PermissionRequested += (_, args) => args.State = CoreWebView2PermissionState.Deny;
            // Downloads need an explicit, reviewed desktop design; they are
            // disabled in this prototype, including chat export.
            core.DownloadStarting += (_, args) => args.Cancel = true;
            core.NavigationCompleted += OnNavigationCompleted;
            core.ProcessFailed += (_, args) =>
            {
                if (!closing.IsCancellationRequested &&
                    (args.ProcessFailedKind == CoreWebView2ProcessFailedKind.BrowserProcessExited ||
                     args.ProcessFailedKind == CoreWebView2ProcessFailedKind.RenderProcessExited))
                    Fail("The desktop web view stopped unexpectedly. Restart ISA.");
            };
            core.Navigate(DesktopPolicy.Home.AbsoluteUri);
        }
        catch (OperationCanceledException) when (closing.IsCancellationRequested) { }
        catch (WebView2RuntimeNotFoundException)
        {
            if (!closing.IsCancellationRequested)
                Fail("Microsoft Edge WebView2 Runtime is required. Install it and restart ISA.");
        }
        catch (Exception error)
        {
            if (!closing.IsCancellationRequested) Fail(error.Message);
        }
    }

    private async void OnNavigationCompleted(object? sender, CoreWebView2NavigationCompletedEventArgs args)
    {
        try
        {
            // A blocked external link is cancelled intentionally, not a
            // reason to close an otherwise healthy desktop window.
            if (!smokeTest && !args.IsSuccess && args.WebErrorStatus == CoreWebView2WebErrorStatus.OperationCanceled) return;
            if (!args.IsSuccess || !DesktopPolicy.IsAllowedNavigation(webView.CoreWebView2.Source))
            {
                Fail($"Desktop navigation failed: {args.WebErrorStatus}");
                return;
            }
            if (!smokeTest) return;
            // The minimal HTML root is replaced by layout.js; require actual
            // interface elements, not just a successful HTTP response.
            string result = await webView.CoreWebView2.ExecuteScriptAsync(
                "Boolean(document.querySelector('.app') && document.getElementById('sidebar') && document.getElementById('newTask'))");
            if (closing.IsCancellationRequested) return;
            if (result != "true")
            {
                Fail("Desktop interface was not mounted.");
                return;
            }
            ExitCode = 0;
            Close();
        }
        catch (Exception error)
        {
            if (!closing.IsCancellationRequested) Fail("Desktop smoke test failed: " + error.Message);
        }
    }

    private void Fail(string message)
    {
        if (reportingFailure || stopping || closing.IsCancellationRequested) return;
        reportingFailure = true;
        ExitCode = 1;
        if (smokeTest) Console.Error.WriteLine(message);
        else MessageBox.Show(this, message, "ISA could not start", MessageBoxButtons.OK, MessageBoxIcon.Error);
        Close();
    }

    private async Task StartBackendAsync(CancellationToken cancellationToken)
    {
        string executable = Path.Combine(AppContext.BaseDirectory, "isa.exe");
        if (!File.Exists(executable))
            throw new FileNotFoundException("The bundled ISA backend (isa.exe) is missing.");

        var info = new ProcessStartInfo(executable)
        {
            WorkingDirectory = AppContext.BaseDirectory,
            UseShellExecute = false,
            CreateNoWindow = true,
            RedirectStandardOutput = true,
        };
        // Do not inherit configuration that could enable local actions,
        // expose the server, or select a workspace without user approval.
        DesktopPolicy.ConfigureBackend(info);

        backend = new Process { StartInfo = info, EnableRaisingEvents = true };
        backend.Exited += BackendExited;
        if (!backend.Start()) throw new InvalidOperationException("Could not start the ISA backend.");

        // Only navigate after our own backend reports that it bound the exact
        // fixed address. An occupied port makes the backend exit instead.
        string? line = await backend.StandardOutput.ReadLineAsync(cancellationToken)
            .AsTask().WaitAsync(TimeSpan.FromSeconds(15), cancellationToken);
        if (line != DesktopPolicy.StartMessage || backend.HasExited)
            throw new InvalidOperationException("ISA could not bind its local port. Close other instances and try again.");
        backendReady = true;
        // If the child exited between the first check and enabling the event
        // handler, report failure rather than continuing to the WebView.
        if (backend.HasExited)
            throw new InvalidOperationException("The ISA backend stopped during startup.");
    }

    private static void RestrictNavigation(object? sender, CoreWebView2NavigationStartingEventArgs args)
    {
        if (!DesktopPolicy.IsAllowedNavigation(args.Uri)) args.Cancel = true;
    }

    private void BackendExited(object? sender, EventArgs e)
    {
        if (!backendReady || stopping || closing.IsCancellationRequested || IsDisposed || !IsHandleCreated) return;
        try
        {
            BeginInvoke((Action)(() =>
            {
                if (stopping || IsDisposed || closing.IsCancellationRequested) return;
                Fail("The ISA backend stopped. The window will close.");
            }));
        }
        catch (InvalidOperationException) { /* The window is already closing. */ }
    }

    private void OnClosing(object? sender, FormClosingEventArgs e)
    {
        stopping = true;
        closing.Cancel();
        smokeTimer?.Stop();
        smokeTimer?.Dispose();
        if (backend is not null)
        {
            try
            {
                if (!backend.HasExited) backend.Kill(entireProcessTree: true);
                backend.WaitForExit(3000);
            }
            catch (InvalidOperationException) { }
            catch (System.ComponentModel.Win32Exception) { }
            backend.Dispose();
            backend = null;
        }
        webView.Dispose();
        if (smokeProfile is not null)
        {
            try { Directory.Delete(smokeProfile, recursive: true); }
            catch (IOException) { /* WebView2 may still be releasing files. */ }
            catch (UnauthorizedAccessException) { }
        }
    }
}
