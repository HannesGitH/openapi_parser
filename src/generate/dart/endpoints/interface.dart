abstract interface class JsonRequestHandler {
  Future<Map<String, dynamic>> handle({
    required APIRequestMethod method,
    required String path,
    Map<String, dynamic> body,
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

  Future<Map<String, dynamic>> handle({
    required APIRequestMethod method,
    Map<String, String> params = const {},
    Map<String, dynamic> body = const {},
  }) {
    final pathString = interpolator.interpolate(path, params);
    return handler.handle(method: method, path: pathString, body: body);
  }
}
