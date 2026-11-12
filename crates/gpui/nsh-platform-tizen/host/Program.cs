// The C# host Tizen's app model requires for a third-party native app: Main makes one P/Invoke into
// libnsh.so (nsh-platform-tizen), which runs ui_app_main, the lifecycle and the main loop itself and
// returns only at exit. No TizenFX, no other P/Invoke, nothing per frame or per event crosses into
// managed code.
using System;
using System.Runtime.InteropServices;

namespace Nsh.Tizen.Host
{
    internal static class Program
    {
        private static unsafe int Main(string[] args)
        {
            // argv[0] is empty (as for flutter-tizen's host); argv is NULL-terminated.
            var argv = stackalloc IntPtr[args.Length + 2];
            argv[0] = Marshal.StringToCoTaskMemUTF8(string.Empty);
            for (int i = 0; i < args.Length; i++)
            {
                argv[i + 1] = Marshal.StringToCoTaskMemUTF8(args[i]);
            }
            argv[args.Length + 1] = IntPtr.Zero;
            return nsh_tizen_main(args.Length + 1, argv);
        }

        [DllImport("libnsh.so", ExactSpelling = true)]
        private static extern unsafe int nsh_tizen_main(int argc, IntPtr* argv);
    }
}
