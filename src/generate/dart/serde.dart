// in dart we cannot enforce our classes to have a fromJson method
import 'dart:convert';

abstract interface class APISerde {
  /// either a Map<String, dynamic> or a List<dynamic> or a String or a int or a double or a bool or null
  /// in this case probably only Map<String, dynamic> and String
  dynamic toJson();
}

extension APISerdeExtension<T extends APISerde> on T {
  String toJsonStr() {
    return jsonEncode(this.toJson());
  }
}

class UnreachableError extends Error {
  final String? message;
  UnreachableError(this.message);
}

class UnknownAPIObject implements APISerde {
  const UnknownAPIObject({this.rawValue});

  final dynamic rawValue;

  @override
  dynamic toJson() => {};

  factory UnknownAPIObject.fromJson(dynamic json) =>
      UnknownAPIObject(rawValue: json);
}
