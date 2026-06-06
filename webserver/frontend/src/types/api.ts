export interface ApiError {
  code: string;
  message: string;
  retryable: boolean;
  details?: unknown;
}

export interface ApiResponse<T> {
  success: boolean;
  data?: T;
  error?: ApiError;
}

export class StructuredApiError extends Error {
  public code: string;
  public retryable: boolean;
  public details: unknown;
  public status?: number;

  constructor(message: string, code: string, retryable: boolean = false, details?: unknown, status?: number) {
    super(message);
    this.name = 'StructuredApiError';
    this.code = code;
    this.retryable = retryable;
    this.details = details;
    this.status = status;
  }
}
