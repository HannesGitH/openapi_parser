abstract interface class JsonRequestHandler {
  Future<dynamic> handle({
    required APIRequestMethod method,
    required String path,
    Map<String, String> params = const {},
    dynamic body,
  });
}

typedef APIRequestLeafDeps = JsonRequestHandler;

enum APIRequestMethod {
  get,
  post,
  put,
  delete,
  patch,
  options,
  head,
}

extension APIPathName on APIPathEnum {
  String get path => this.toJson();
}

abstract class APIPath {
  final APIPathEnum path;
  final String interpolatedPath;
  final JsonRequestHandler handler;
  APIPath(
      {required this.path,
      required this.interpolatedPath,
      required this.handler});

  Future<dynamic> handle({
    required APIRequestMethod method,
    Map<String, String> params = const {},
    dynamic body = const {},
  }) {
    return handler.handle(
        method: method, path: interpolatedPath, params: params, body: body);
  }
}

//TODO: this is the root API
class API extends APIHasPath {
  final APIRequestLeafDeps deps;
  API({required JsonRequestHandler handler}) : deps = handler;

  APIrootFrag_ get fragmented => APIrootFrag_(deps: this.deps, parent: this);

  @override
  String get path => '';
}

abstract interface class APIHasPath {
  String get path;
}

abstract class APIWithParent implements APIHasPath {
  final APIHasPath parent;
  final String ownFragment;
  final APIRequestLeafDeps deps;

  APIWithParent(
      {required this.parent, required this.ownFragment, required this.deps});

  @override
  String get path => "${parent.path}/$ownFragment";
}
