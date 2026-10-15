# Test cases for sloptopop.errors.python-except-pass (semgrep --test format).


def bare(order):
    # ruleid: sloptopop.errors.python-except-pass
    try:
        save(order)
    except:
        pass


def broad(order):
    # ruleid: sloptopop.errors.python-except-pass
    try:
        save(order)
    except Exception:
        pass


def broad_bound(order):
    # ruleid: sloptopop.errors.python-except-pass
    try:
        save(order)
    except Exception as e:
        # ignore
        pass


def base(order):
    # ruleid: sloptopop.errors.python-except-pass
    try:
        save(order)
    except BaseException:
        pass


def after_specific(order):
    # ruleid: sloptopop.errors.python-except-pass
    try:
        save(order)
    except ValueError:
        log.warning("bad order")
    except Exception:
        pass


def specific(order):
    # ok: sloptopop.errors.python-except-pass
    try:
        save(order)
    except KeyError:
        pass


def logged(order):
    # ok: sloptopop.errors.python-except-pass
    try:
        save(order)
    except Exception:
        log.exception("save failed")
        raise
