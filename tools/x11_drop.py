"""Scoped, bounded XDND source for owned X11 test windows (not a desktop helper)."""
import ctypes as c
import time


def drop(window, paths):
    payload = ('\r\n'.join(p.resolve().as_uri() for p in paths) + '\r\n').encode()
    if len(payload) > 65536:
        raise ValueError('test drop too large')
    class Data(c.Union):
        _fields_ = [('l', c.c_long * 5), ('b', c.c_char * 20)]
    class Client(c.Structure):
        _fields_ = [('type', c.c_int), ('serial', c.c_ulong), ('send_event', c.c_int),
                   ('display', c.c_void_p), ('window', c.c_ulong), ('message_type', c.c_ulong),
                   ('format', c.c_int), ('data', Data)]
    class Request(c.Structure):
        _fields_ = [('type', c.c_int), ('serial', c.c_ulong), ('send_event', c.c_int),
                   ('display', c.c_void_p), ('owner', c.c_ulong), ('requestor', c.c_ulong),
                   ('selection', c.c_ulong), ('target', c.c_ulong), ('property', c.c_ulong), ('time', c.c_ulong)]
    class Notify(c.Structure):
        _fields_ = [('type', c.c_int), ('serial', c.c_ulong), ('send_event', c.c_int),
                   ('display', c.c_void_p), ('requestor', c.c_ulong), ('selection', c.c_ulong),
                   ('target', c.c_ulong), ('property', c.c_ulong), ('time', c.c_ulong)]
    class Event(c.Union):
        _fields_ = [('client', Client), ('request', Request), ('notify', Notify), ('pad', c.c_long * 24)]
    x = c.CDLL('libX11.so.6')
    signatures = {
        'XOpenDisplay': (c.c_void_p, [c.c_char_p]),
        'XDefaultRootWindow': (c.c_ulong, [c.c_void_p]),
        'XCreateSimpleWindow': (c.c_ulong, [c.c_void_p, c.c_ulong, c.c_int, c.c_int, c.c_uint, c.c_uint, c.c_uint, c.c_ulong, c.c_ulong]),
        'XInternAtom': (c.c_ulong, [c.c_void_p, c.c_char_p, c.c_int]),
        'XSetSelectionOwner': (c.c_int, [c.c_void_p, c.c_ulong, c.c_ulong, c.c_ulong]),
        'XSendEvent': (c.c_int, [c.c_void_p, c.c_ulong, c.c_int, c.c_long, c.POINTER(Event)]),
        'XChangeProperty': (c.c_int, [c.c_void_p, c.c_ulong, c.c_ulong, c.c_ulong, c.c_int, c.c_int, c.c_void_p, c.c_int]),
        'XPending': (c.c_int, [c.c_void_p]),
        'XNextEvent': (c.c_int, [c.c_void_p, c.POINTER(Event)]),
        'XTranslateCoordinates': (c.c_int, [c.c_void_p, c.c_ulong, c.c_ulong, c.c_int, c.c_int, c.POINTER(c.c_int), c.POINTER(c.c_int), c.POINTER(c.c_ulong)]),
        'XFlush': (c.c_int, [c.c_void_p]),
        'XDestroyWindow': (c.c_int, [c.c_void_p, c.c_ulong]),
        'XCloseDisplay': (c.c_int, [c.c_void_p]),
    }
    for name, (restype, argtypes) in signatures.items():
        getattr(x, name).restype = restype
        getattr(x, name).argtypes = argtypes
    display = x.XOpenDisplay(None)
    if not display:
        raise RuntimeError('test display unavailable')
    source = None
    try:
        root = x.XDefaultRootWindow(display)
        source = x.XCreateSimpleWindow(display, root, 0, 0, 1, 1, 0, 0, 0)
        atom = lambda name: x.XInternAtom(display, name.encode(), 0)
        selection, uri, copy = atom('XdndSelection'), atom('text/uri-list'), atom('XdndActionCopy')
        x.XSetSelectionOwner(display, selection, source, 0)
        target = int(window)
        def send(name, values):
            event = Event()
            event.client.type = 33
            event.client.display = display
            event.client.window = target
            event.client.message_type = atom(name)
            event.client.format = 32
            for i, value in enumerate(values):
                event.client.data.l[i] = value
            if not x.XSendEvent(display, target, 0, 0, c.byref(event)):
                raise RuntimeError('XDND send failed')
            x.XFlush(display)
        rx, ry, child = c.c_int(), c.c_int(), c.c_ulong()
        x.XTranslateCoordinates(display, target, root, 180, 160, c.byref(rx), c.byref(ry), c.byref(child))
        send('XdndEnter', [source, 5 << 24, uri, 0, 0])
        send('XdndPosition', [source, 0, ((rx.value & 65535) << 16) | (ry.value & 65535), 0, copy])
        status, finished, sent = atom('XdndStatus'), atom('XdndFinished'), False
        deadline = time.monotonic() + 4
        while time.monotonic() < deadline:
            if not x.XPending(display):
                time.sleep(.01)
                continue
            event = Event()
            x.XNextEvent(display, c.byref(event))
            if event.client.type == 33:
                if event.client.message_type == status and not sent:
                    if not event.client.data.l[1] & 1:
                        raise RuntimeError('XDND rejected')
                    send('XdndDrop', [source, 0, 0, 0, 0])
                    sent = True
                elif event.client.message_type == finished:
                    return
            elif event.request.type == 30:
                request = event.request
                prop = request.property or request.target
                data = c.create_string_buffer(payload)
                x.XChangeProperty(display, request.requestor, prop, uri, 8, 0, data, len(payload))
                reply = Event()
                reply.notify = Notify(31, 0, 1, display, request.requestor, request.selection, request.target, prop, request.time)
                x.XSendEvent(display, request.requestor, 0, 0, c.byref(reply))
                x.XFlush(display)
        raise RuntimeError('XDND deadline')
    finally:
        if source:
            x.XDestroyWindow(display, source)
        x.XCloseDisplay(display)
