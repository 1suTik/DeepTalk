import { useEffect, useRef } from "react";

/** 主窗口背景光斑：跟随指针位置，纯装饰、不拦截交互。 */
export function MouseGlow() {
  const glowRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    const glow = glowRef.current;
    if (!glow) return;

    let frameId: number | undefined;
    let x = 0;
    let y = 0;

    const scheduleFrame = () => {
      if (frameId !== undefined) return;
      frameId = requestAnimationFrame(() => {
        frameId = undefined;
        glow.style.setProperty("--spot-x", `${x}px`);
        glow.style.setProperty("--spot-y", `${y}px`);
        glow.classList.add("mouse-glow--visible");
      });
    };

    const onPointerMove = (event: PointerEvent) => {
      x = event.clientX;
      y = event.clientY;
      scheduleFrame();
    };

    window.addEventListener("pointermove", onPointerMove, { passive: true });
    return () => {
      window.removeEventListener("pointermove", onPointerMove);
      if (frameId !== undefined) cancelAnimationFrame(frameId);
    };
  }, []);

  return (
    <div
      ref={glowRef}
      className="mouse-glow"
      data-testid="mouse-glow"
      aria-hidden="true"
    />
  );
}
