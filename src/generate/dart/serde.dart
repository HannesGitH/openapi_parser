// in dart we cannot enforce our classes to have a fromJson method
import 'dart:convert';

abstract interface class BEAMSerde {
  /// either a Map<String, dynamic> or a List<dynamic> or a String or a int or a double or a bool or null
  /// in this case probably only Map<String, dynamic> and String
  dynamic toJson();
}

extension BEAMSerdeExtension<T extends BEAMSerde> on T {
  String toJsonStr() {
    return jsonEncode(this.toJson());
  }
}

class UnreachableError extends Error {
  final String? message;
  UnreachableError(this.message);
}

class UnknownBEAMObject implements BEAMSerde {
  const UnknownBEAMObject({this.rawValue});

  final dynamic rawValue;

  @override
  dynamic toJson() => {};

  factory UnknownBEAMObject.fromJson(dynamic json) =>
      UnknownBEAMObject(rawValue: json);
}
