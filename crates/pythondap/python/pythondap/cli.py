from IPython import embed
import argparse

from pythondap.session import DebugSession


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("-b", "--breakpoint", type=int, nargs="*", default=[])
    parser.add_argument("-f", "--file", required=False)
    parser.add_argument("launch_configuration")
    parser.add_argument("-n", "--configuration")
    args = parser.parse_args()

    ns = DebugSession(  # noqa: F841
        breakpoints=args.breakpoint,
        file=args.file,
        config_path=args.launch_configuration,
        config_name=args.configuration,
    )
    embed()
