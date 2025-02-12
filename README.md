# mandelbrot


## Head scratchers

### Scaling factor incorrect between monitors

Have a monitor (A monitor) using scaling factor 1.5 and one using 1.0 (B monitor) for example.
IF A is set as the primary monitor then the image will be displayed correctly, but will appear
zoomed out by 50% if the window is dragged to B.
If B is set as the primary monitor then the image will be displayed correctly, but will appear
zoomed in by 50% if the window is dragged to A.

The scaling factor is updated correctly, the logical, the physical resolutions are all correct.
The mandelbrot set sample coordinates are calculated in the shader as the percentage of the window
we have traveled.
Let us represent window on monitor A as WA, and on B as WB. This is the same window that we can drag between
both, and they change their resolution according to the DPI settings of the given monitor.

If we have window WA on resolution 1536 x 1152 and window WB on resolution
1024 x 768, and we are passing the physical size of the monitor for sampling the
mandlebrot set then my testing shows the following behavior:

|Window created on (default monitor) \ Window dragged to| A | B |
|---|---|----|
|A| OK | Zoomed in 50% |
|B| Zoomed out 50% | OK |

If instead we pass the logical resolution for sampling:

|Window created on (default monitor) \ Window dragged to| A | B |
|---|---|----|
|A| OK | OK |
|B| Zoomed out 50% | Zoomed out 50% |

It is as if during window creation something gets set in WebGPU as an internal
resolution for the window texture or something, which equals the physical
resolution of the window that was initially created.
This is further confirmed by passing the resolution of the window statically, based
on which monitor was set as the primary one. In these cases, the image will be
correctly displayed.
Not sure what is happening exactly, but this seems to approximate the issue.
The shader will receive the same resolution as the window needed on the display
it was initially constructed on.

Bah and the fix is so simple. The GPU remembered the original size because it was never
notified that the resolution changed! Wininit knows about the new resolution as that is
handling the window, but the GPU does not. It has to be told exactly what it should do.
In this case, when a window is resized, the underlying surface has to be remade as that
seems to be component representing the texture it is drawing to.
