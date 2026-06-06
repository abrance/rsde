import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { requestJson } from './api';
import { StructuredApiError } from '../types/api';

describe('requestJson', () => {
  const originalFetch = globalThis.fetch;

  beforeEach(() => {
    globalThis.fetch = vi.fn();
  });

  afterEach(() => {
    globalThis.fetch = originalFetch;
    vi.resetAllMocks();
  });

  it('parses a successful ApiResponse', async () => {
    const mockResponse = {
      ok: true,
      status: 200,
      json: async () => ({
        success: true,
        data: { foo: 'bar' }
      })
    };
    (globalThis.fetch as any).mockResolvedValue(mockResponse);

    const result = await requestJson<{ foo: string }>('/api/test');
    
    expect(globalThis.fetch).toHaveBeenCalledWith('/api/test', undefined);
    expect(result).toEqual({ foo: 'bar' });
  });

  it('throws StructuredApiError when success is true but data is missing', async () => {
    const mockResponse = {
      ok: true,
      status: 200,
      json: async () => ({
        success: true
      })
    };
    (globalThis.fetch as any).mockResolvedValue(mockResponse);

    const error = await requestJson('/api/test').catch(e => e);
    
    expect(error).toBeInstanceOf(StructuredApiError);
    const err = error as StructuredApiError;
    expect(err.code).toBe('MISSING_DATA');
    expect(err.message).toBe('API succeeded but data is missing');
    expect(err.status).toBe(200);
  });

  it('throws StructuredApiError when ok is false and error payload is present', async () => {
    const mockResponse = {
      ok: false,
      status: 404,
      json: async () => ({
        success: false,
        error: {
          code: 'NODE_NOT_FOUND',
          message: 'Node not found in DB',
          retryable: false,
          details: { id: 'node-1' }
        }
      })
    };
    (globalThis.fetch as any).mockResolvedValue(mockResponse);

    const error = await requestJson('/api/test').catch(e => e);
    
    expect(error).toBeInstanceOf(StructuredApiError);
    const err = error as StructuredApiError;
    expect(err.code).toBe('NODE_NOT_FOUND');
    expect(err.message).toBe('Node not found in DB');
    expect(err.retryable).toBe(false);
    expect(err.details).toEqual({ id: 'node-1' });
    expect(err.status).toBe(404);
  });

  it('throws StructuredApiError when HTTP status is 200 but success is false', async () => {
    const mockResponse = {
      ok: true,
      status: 200,
      json: async () => ({
        success: false,
        error: {
          code: 'LOGICAL_ERROR',
          message: 'A logical error occurred',
          retryable: true,
          details: { reason: 'bad input' }
        }
      })
    };
    (globalThis.fetch as any).mockResolvedValue(mockResponse);

    const error = await requestJson('/api/test').catch(e => e);
    
    expect(error).toBeInstanceOf(StructuredApiError);
    const err = error as StructuredApiError;
    expect(err.code).toBe('LOGICAL_ERROR');
    expect(err.message).toBe('A logical error occurred');
    expect(err.retryable).toBe(true);
    expect(err.details).toEqual({ reason: 'bad input' });
    expect(err.status).toBe(200);
  });

  it('throws a generic StructuredApiError on HTTP error without structured JSON error', async () => {
    const mockResponse = {
      ok: false,
      status: 500,
      json: async () => { throw new Error('Not JSON'); }
    };
    (globalThis.fetch as any).mockResolvedValue(mockResponse);

    const error = await requestJson('/api/test').catch(e => e);
    
    expect(error).toBeInstanceOf(StructuredApiError);
    const err = error as StructuredApiError;
    expect(err.code).toBe('HTTP_ERROR_500');
    expect(err.message).toBe('HTTP error 500');
    expect(err.retryable).toBe(false);
    expect(err.details).toBeUndefined();
    expect(err.status).toBe(500);
  });

  it('throws a generic StructuredApiError when response is OK but JSON parsing fails', async () => {
    const mockResponse = {
      ok: true,
      status: 200,
      json: async () => { throw new Error('Invalid JSON'); }
    };
    (globalThis.fetch as any).mockResolvedValue(mockResponse);

    const error = await requestJson('/api/test').catch(e => e);
    
    expect(error).toBeInstanceOf(StructuredApiError);
    const err = error as StructuredApiError;
    expect(err.code).toBe('INVALID_JSON');
    expect(err.message).toBe('Failed to parse response JSON');
    expect(err.retryable).toBe(false);
    expect(err.details).toBeUndefined();
    expect(err.status).toBe(200);
  });

  it('throws a generic StructuredApiError on fetch network failure', async () => {
    (globalThis.fetch as any).mockRejectedValue(new Error('Network offline'));

    const error = await requestJson('/api/test').catch(e => e);
    
    expect(error).toBeInstanceOf(StructuredApiError);
    const err = error as StructuredApiError;
    expect(err.code).toBe('NETWORK_ERROR');
    expect(err.message).toContain('Network offline');
    expect(err.retryable).toBe(true);
    expect(err.details).toBeUndefined();
    expect(err.status).toBeUndefined();
  });
});
