import { useEffect, useMemo, useRef, useState } from 'react';
import { QuorumMatrixRenderer } from './QuorumMatrixRenderer.js';
import { cellForPosition } from '../../analytics/matrix/quorumMatrixModel.js';

function pickCell(matrix, renderer, clientX, clientY) {
  const rect = renderer.canvas.getBoundingClientRect();
  const width = rect.width || 1;
  const height = rect.height || 1;
  const stride = renderer.cellSize + renderer.gap;
  const half = (matrix.size * stride) / 2;
  const aspect = width / height;
  const ndcX = ((clientX - rect.left) / width) * 2 - 1;
  const ndcY = -((clientY - rect.top) / height) * 2 + 1;
  const worldX = ndcX * half * aspect;
  const worldY = ndcY * half;
  const column = Math.round(worldX / stride + (matrix.size - 1) / 2);
  const row = Math.round((matrix.size - 1) / 2 - worldY / stride);
  return cellForPosition(matrix, row, column);
}

export default function QuorumMatrixCanvas({ matrix, onHoverCell, cellSize = 1, gap = 0.08 }) {
  const canvasRef = useRef(null);
  const rendererRef = useRef(null);
  const matrixRef = useRef(matrix);
  const hoverRef = useRef(null);
  const [fps, setFps] = useState(0);

  useEffect(() => { matrixRef.current = matrix; }, [matrix]);

  const renderer = useMemo(() => {
    if (!canvasRef.current) return null;
    return new QuorumMatrixRenderer({ canvas: canvasRef.current, cellSize, gap });
  }, [cellSize, gap]);

  useEffect(() => {
    rendererRef.current = renderer;
    if (!renderer) return undefined;
    const canvas = renderer.canvas;
    const parent = canvas.parentElement;
    const resize = () => renderer.resize(parent.clientWidth || 800, parent.clientHeight || 600);
    const observer = new ResizeObserver(resize);
    observer.observe(parent);
    resize();

    let raf = 0;
    let frames = 0;
    let last = performance.now();
    const loop = () => {
      renderer.render(matrixRef.current);
      frames += 1;
      const now = performance.now();
      if (now - last >= 1000) {
        setFps(Math.round((frames * 1000) / (now - last)));
        frames = 0;
        last = now;
      }
      raf = requestAnimationFrame(loop);
    };
    raf = requestAnimationFrame(loop);

    return () => {
      cancelAnimationFrame(raf);
      observer.disconnect();
      renderer.dispose();
      rendererRef.current = null;
    };
  }, [renderer]);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return undefined;
    const onMove = (event) => {
      const rendererInstance = rendererRef.current;
      if (!rendererInstance) return;
      const cell = pickCell(matrixRef.current, rendererInstance, event.clientX, event.clientY);
      hoverRef.current = cell;
      rendererInstance.highlight = cell;
      onHoverCell?.(cell);
    };
    const onLeave = () => {
      const rendererInstance = rendererRef.current;
      if (rendererInstance) rendererInstance.highlight = null;
      hoverRef.current = null;
      onHoverCell?.(null);
    };
    canvas.addEventListener('pointermove', onMove);
    canvas.addEventListener('pointerleave', onLeave);
    return () => {
      canvas.removeEventListener('pointermove', onMove);
      canvas.removeEventListener('pointerleave', onLeave);
    };
  }, [onHoverCell]);

  return (
    <div className="matrix-host" role="img" aria-label="Interactive quorum matrix">
      <canvas ref={canvasRef} />
      <span className="matrix-fps" aria-live="off">{fps} fps</span>
    </div>
  );
}
