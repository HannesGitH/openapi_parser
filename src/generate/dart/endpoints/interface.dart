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
  }) {
    return handler
        .handle(
          method: method,
          path: interpolatedPath,
          params: params,
          body: body,
          expectedResponseType: expectedResponseType,
        )
        .then((response) {
          handler.cache?.storeInCache(
            response: response,
            method: method,
            path: interpolatedPath,
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
  }) {
    return BEAMCachedResponse<dynamic>(
      upstreamFuture: handle(
        method: method,
        params: params,
        body: body,
        expectedResponseType: expectedResponseType,
      ),
      cachedFuture: handler.cache?.fetchFromCache(
        method: method,
        path: interpolatedPath,
        params: params,
        body: body,
        expectedResponseType: expectedResponseType,
      ),
    );
  }
}

//TODO: this is the root BEAM
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

class BEAMCachedResponse<T> {
  BEAMCachedResponse({
    required Future<T> upstreamFuture,
    required Future<T>? cachedFuture,
  }) : _upstreamFuture = upstreamFuture,
       _cachedFuture = cachedFuture,
       _streamController = StreamController<T>() {
    _cachedFuture?.then((value) {
      _streamController.add(value);
    });
    _upstreamFuture.then((value) async {
      // make sure upstream always comes after cached in the stream, st we never override new data with old data
      await _cachedFuture;
      _streamController.add(value);
      _streamController.close();
    });
  }

  final Future<T> _upstreamFuture;
  final Future<T>? _cachedFuture;
  final StreamController<T> _streamController;

  Future<T> get first => Future.any(
    // todo: update with null-aware-elements
    _cachedFuture != null
        ? [_upstreamFuture, _cachedFuture]
        : [_upstreamFuture],
  );

  Future<T> get actual => _upstreamFuture;

  Stream<T> get stream => _streamController.stream;

  BEAMCachedResponse<T2> then<T2>(T2 Function(T) onValue) =>
      BEAMCachedResponse<T2>(
        upstreamFuture: _upstreamFuture.then(onValue),
        cachedFuture: _cachedFuture?.then(onValue),
      );
}

abstract class BEAMCacheHandler {
  Future<dynamic>? fetchFromCache({
    required BEAMRequestMethod method,
    required String path,
    Map<String, String> params = const {},
    dynamic body,
    BEAMExpectedResponseType expectedResponseType =
        BEAMExpectedResponseType.json,
  });

  Future<dynamic> storeInCache({
    required dynamic response,
    required BEAMRequestMethod method,
    required String path,
    Map<String, String> params = const {},
    dynamic body,
    BEAMExpectedResponseType expectedResponseType =
        BEAMExpectedResponseType.json,
  });
}
