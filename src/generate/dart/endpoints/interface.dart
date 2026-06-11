abstract interface class JsonRequestHandler {
  Future<dynamic> handle({
    required BEAMRequestMethod method,
    required String path,
    Map<String, String> params = const {},
    dynamic body,
    BEAMExpectedResponseType expectedResponseType =
        BEAMExpectedResponseType.json,
  });

  BEAMCacheHandler? cache;
}

/// A mutable out-parameter the request machinery passes down so a
/// status-code-aware handler can report the HTTP status code of a response.
///
/// Generated multi-status endpoints (those returning a
/// [BeamStatusCodeResponse]) create one of these per call; when the handler
/// fills [statusCode] the response is decoded deterministically via
/// `fromCode`, otherwise decoding falls back to the (discouraged) `fromJson`.
class BeamStatusCodeRef {
  int? statusCode;
}

/// Optional, additive capability a [JsonRequestHandler] MAY also implement to
/// report the HTTP status code of a response.
///
/// This is fully backwards compatible: handlers that only implement
/// [JsonRequestHandler] keep working unchanged, and multi-status endpoints
/// simply fall back to `fromJson` for them. Handlers that implement this get
/// deterministic, status-code-driven decoding via `fromCode`.
abstract interface class BeamStatusCodeAwareHandler
    implements JsonRequestHandler {
  /// Like [JsonRequestHandler.handle], but also assigns the response's HTTP
  /// status code to [statusCodeRef] before the returned future completes.
  Future<dynamic> handleWithStatusCode({
    required BEAMRequestMethod method,
    required String path,
    required BeamStatusCodeRef statusCodeRef,
    Map<String, String> params = const {},
    dynamic body,
    BEAMExpectedResponseType expectedResponseType =
        BEAMExpectedResponseType.json,
  });
}

enum BEAMExpectedResponseType { json, binary }

typedef BEAMRequestLeafDeps = JsonRequestHandler;

enum BEAMRequestMethod { get, post, put, delete, patch, options, head }

extension BEAMPathName on BEAMPathEnum {
  String get path => this.toJson();
}

abstract class BEAMPath {
  final BEAMPathEnum path;
  final String interpolatedPath;
  final JsonRequestHandler handler;
  BEAMPath({
    required this.path,
    required this.interpolatedPath,
    required this.handler,
  });

  Future<dynamic> handle({
    required BEAMRequestMethod method,
    Map<String, String> params = const {},
    dynamic body = const {},
    BEAMExpectedResponseType expectedResponseType =
        BEAMExpectedResponseType.json,
    BeamStatusCodeRef? statusCodeRef,
  }) {
    final h = handler;
    // Use the status-code-aware path only when a ref was requested AND the
    // handler supports it; otherwise fall back to the plain `handle`.
    final Future<dynamic> upstream =
        (statusCodeRef != null && h is BeamStatusCodeAwareHandler)
        ? h.handleWithStatusCode(
            method: method,
            path: interpolatedPath,
            statusCodeRef: statusCodeRef,
            params: params,
            body: body,
            expectedResponseType: expectedResponseType,
          )
        : h.handle(
            method: method,
            path: interpolatedPath,
            params: params,
            body: body,
            expectedResponseType: expectedResponseType,
          );
    return upstream.then((response) {
      handler.cache?.storeInCache(
        response: response,
        method: method,
        interpolatedPath: interpolatedPath,
        path: path,
        params: params,
        body: body,
        expectedResponseType: expectedResponseType,
      );
      return response;
    });
  }

  BEAMCachedResponse<dynamic> handleCached({
    required BEAMRequestMethod method,
    Map<String, String> params = const {},
    dynamic body = const {},
    BEAMExpectedResponseType expectedResponseType =
        BEAMExpectedResponseType.json,
    BeamStatusCodeRef? statusCodeRef,
  }) {
    return BEAMCachedResponse<dynamic>(
      upstreamFuture: handle(
        method: method,
        params: params,
        body: body,
        expectedResponseType: expectedResponseType,
        statusCodeRef: statusCodeRef,
      ),
      cachedFuture: handler.cache?.fetchFromCache(
        method: method,
        interpolatedPath: interpolatedPath,
        path: path,
        params: params,
        body: body,
        expectedResponseType: expectedResponseType,
      ),
    );
  }
}

class BEAM extends BEAMHasPath {
  final BEAMRequestLeafDeps deps;
  BEAM({required JsonRequestHandler handler}) : deps = handler;

  BEAMrootFrag_ get fragmented => BEAMrootFrag_(deps: this.deps, parent: this);

  @override
  String get path => '';
}

abstract interface class BEAMHasPath {
  String get path;
}

abstract class BEAMWithParent implements BEAMHasPath {
  final BEAMHasPath parent;
  final String ownFragment;
  final BEAMRequestLeafDeps deps;

  BEAMWithParent({
    required this.parent,
    required this.ownFragment,
    required this.deps,
  });

  @override
  String get path => "${parent.path}/$ownFragment";
}

sealed class BeamCachedResponseError {
  final dynamic error;

  @override
  String toString() => '$runtimeType: $error';

  BeamCachedResponseError(this.error);
}

final class BeamCacheError extends BeamCachedResponseError {
  BeamCacheError(super.error);
}

final class BeamUpstreamError extends BeamCachedResponseError {
  BeamUpstreamError(super.error);
}

class BEAMCachedResponse<T> {
  BEAMCachedResponse({
    required Future<T> upstreamFuture,
    required Future<T>? cachedFuture,
  }) : _upstreamFuture = upstreamFuture,
       _cachedFuture = cachedFuture,
       _streamController = StreamController<T>() {
    _cachedFuture?.then(
      (value) {
        // A cache "miss" (no entry yet) is the default state, not an error.
        // Only emit if we actually got a cached value.
        if (value == null) return;
        _streamController.add(value);
      },
      onError: (error, stackTrace) {
        // Real cache errors (corrupt data, IO failure) still surface here.
        _streamController.addError(BeamCacheError(error), stackTrace);
      },
    );
    _upstreamFuture.then(
      (value) async {
        // make sure upstream always comes after cached in the stream, st we never override new data with old data
        try {
          await _cachedFuture;
        } catch (_) {
          // we dont care, cache miss already handled above
        }
        _streamController.add(value);
        _streamController.close();
      },
      onError: (error, stackTrace) {
        // here, the order is not as important, what to do if cache hit, but upstream fails, is up to the user of beam
        _streamController.addError(BeamUpstreamError(error), stackTrace);
      },
    );
  }

  final Future<T> _upstreamFuture;
  final Future<T>? _cachedFuture;
  final StreamController<T> _streamController;

  Future<T> get first {
    // No cache leg at all? Just mirror upstream directly so callers see the
    // real upstream error (not an AnySuccessError wrapper) on failure.
    if (_cachedFuture == null) return _upstreamFuture;
    final completer = Completer<T>.sync();
    _cachedFuture.then(
      (value) {
        if (!completer.isCompleted) completer.complete(value);
      },
      // A cache failure is NOT a failure of `first` -- the whole point of
      // caching is graceful degradation. Just wait for upstream.
      onError: (_, __) {},
    );
    _upstreamFuture.then(
      (value) {
        if (!completer.isCompleted) completer.complete(value);
      },
      // Upstream failing IS a failure of `first`. Surface the real error
      // directly so typed `catch` clauses at the call site still work, rather
      // than wrapping it in AnySuccessError.
      onError: (error, stackTrace) {
        if (!completer.isCompleted) completer.completeError(error, stackTrace);
      },
    );
    return completer.future;
  }

  Future<T> get actual => _upstreamFuture;

  Stream<T> get stream => _streamController.stream;

  BEAMCachedResponse<T2> then<T2>(FutureOr<T2> Function(T) onValue) =>
      BEAMCachedResponse<T2>(
        upstreamFuture: _upstreamFuture.then(onValue),
        cachedFuture: _cachedFuture?.then(onValue),
      );
}

abstract class BEAMCacheHandler {
  Future<dynamic>? fetchFromCache({
    required BEAMRequestMethod method,
    required String interpolatedPath,
    required BEAMPathEnum path,
    Map<String, String> params = const {},
    dynamic body,
    BEAMExpectedResponseType expectedResponseType =
        BEAMExpectedResponseType.json,
  });

  Future<dynamic> storeInCache({
    required dynamic response,
    required BEAMRequestMethod method,
    required String interpolatedPath,
    required BEAMPathEnum path,
    Map<String, String> params = const {},
    dynamic body,
    BEAMExpectedResponseType expectedResponseType =
        BEAMExpectedResponseType.json,
  });
}

class FutureHelper {
  static Future<T> anySuccess<T>(List<Future<T>> futures) {
    final completer = Completer<T>.sync();
    void onValue(T value) {
      if (!completer.isCompleted) completer.complete(value);
    }

    int failedCount = 0;
    void onError(Object error, StackTrace stack) {
      if (!completer.isCompleted) {
        failedCount++;
        if (failedCount == futures.length) {
          completer.completeError(AnySuccessError(error));
        }
      }
    }

    for (var future in futures) {
      future.then(onValue, onError: onError);
    }
    return completer.future;
  }
}

class AnySuccessError extends Error {
  final Object lastError;
  AnySuccessError(this.lastError);

  @override
  String toString() {
    return 'No successful future found, last error: $lastError';
  }
}
