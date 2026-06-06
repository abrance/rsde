import { ApiResponse, StructuredApiError } from '../types/api';

export async function requestJson<T>(url: string, options?: RequestInit): Promise<T> {
  let response: Response;
  try {
    response = await fetch(url, options);
  } catch (err: unknown) {
    const msg = err instanceof Error ? err.message : 'Network error';
    throw new StructuredApiError(msg, 'NETWORK_ERROR', true);
  }

  let json: unknown;
  try {
    json = await response.json();
  } catch (err: unknown) {
    if (!response.ok) {
      throw new StructuredApiError(`HTTP error ${response.status}`, `HTTP_ERROR_${response.status}`, false, undefined, response.status);
    }
    throw new StructuredApiError('Failed to parse response JSON', 'INVALID_JSON', false, undefined, response.status);
  }

  const apiResponse = json as ApiResponse<T>;

  if (!response.ok || !apiResponse.success) {
    if (apiResponse.error) {
      throw new StructuredApiError(
        apiResponse.error.message,
        apiResponse.error.code,
        apiResponse.error.retryable,
        apiResponse.error.details,
        response.status
      );
    }
    throw new StructuredApiError(`HTTP error ${response.status}`, `HTTP_ERROR_${response.status}`, false, undefined, response.status);
  }

  if (apiResponse.data === undefined) {
    throw new StructuredApiError('API succeeded but data is missing', 'MISSING_DATA', false, undefined, response.status);
  }

  return apiResponse.data as T;
}
