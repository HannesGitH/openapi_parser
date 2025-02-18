abstract interface class JsonRequestHandler {
  Future<dynamic> handle({
    required APIRequestMethod method,
    required String path,
    dynamic body,
  });
}

enum APIRequestMethod {
  get,
  post,
  put,
  delete,
  patch,
  options,
  head,
}

abstract interface class APIPathInterpolator {
  String interpolate(APIPathEnum path, Map<String, String> params);
}

extension APIPathName on APIPathEnum {
  String get path => this.toJson();
}

abstract class APIPath {
  final APIPathEnum path;
  final APIPathInterpolator interpolator;
  final JsonRequestHandler handler;
  APIPath(
      {required this.path, required this.interpolator, required this.handler});

  Future<dynamic> handle({
    required APIRequestMethod method,
    Map<String, String> params = const {},
    dynamic body = const {},
  }) {
    final pathString = interpolator.interpolate(path, params);
    return handler.handle(method: method, path: pathString, body: body);
  }
}

//TODO: this is the root API
class API extends APIHasPath {
  APIrootFragment get root => APIrootFragment(parent: this);
  API();

  APIrootFragment call() => root;

  @override
  String get path => '';
}

abstract interface class APIHasPath {
  String get path;
}

abstract class APIWithParent implements APIHasPath {
  final APIHasPath parent;
  final String ownFragment;

  APIWithParent({required this.parent, required this.ownFragment});

  @override
  String get path => "${parent.path}/$ownFragment";
}
