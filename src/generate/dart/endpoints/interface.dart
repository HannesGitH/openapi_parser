abstract interface class JsonRequestHandler {
  Future<dynamic> handle({
    required BEAMRequestMethod method,
    required String path,
    Map<String, String> params = const {},
    dynamic body,
    BEAMExpectedResponseType expectedResponseType = BEAMExpectedResponseType.json,
  });
}

enum BEAMExpectedResponseType {
  json,
  stream,
  binary,
}

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
    BEAMExpectedResponseType expectedResponseType = BEAMExpectedResponseType.json,
  }) {
    return handler.handle(
      method: method,
      path: interpolatedPath,
      params: params,
      body: body,
      expectedResponseType: expectedResponseType,
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
