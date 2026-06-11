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

/// Implemented by every generated response union that aggregates more than
/// one HTTP status code.
///
/// On top of [BEAMSerde.toJson] and the (non-deterministic, discouraged)
/// `fromJson`, each implementer exposes a static factory matching
/// [BeamStatusCodeResponseParser]:
///
/// ```dart
/// static T? fromCode(int statusCode, dynamic json)
/// ```
///
/// which decodes the variant for a given HTTP status code (or returns null
/// for an unknown one). A request handler that knows the response's status
/// code can therefore decode the correct variant deterministically —
/// `value is BeamStatusCodeResponse` flags the unions that support this —
/// instead of falling back to the ambiguous `fromJson`.
///
/// Note: `fromCode` is necessarily `static` (the value is produced *by* it),
/// so it cannot be a member of this interface; the generated unions wire
/// their `fromCode` tear-off to the handler explicitly.
abstract interface class BeamStatusCodeResponse implements BEAMSerde {}

/// Signature of the static `fromCode` factory every
/// [BeamStatusCodeResponse] provides: maps an HTTP `statusCode` and the
/// decoded `json` body to the matching response variant, or null for an
/// unknown status code.
typedef BeamStatusCodeResponseParser<T extends BeamStatusCodeResponse> =
    T? Function(int statusCode, dynamic json);

class BEAMUnknownValueError extends Error {
  final String? message;
  BEAMUnknownValueError(this.message);
}

class BEAMWrongTypeError extends Error {
  final String? message;
  BEAMWrongTypeError(this.message);

  @override
  toString() => '$BEAMWrongTypeError: $message';
}

class BEAMUnionParseMultiError extends Error {
  final Map<String, Object> errors;
  BEAMUnionParseMultiError(this.errors);

  @override
  String toString() {
    var string = '$BEAMUnionParseMultiError\n';
    for (final key in errors.keys) {
      string += ' - $key: ${errors[key].toString().replaceAll('\n', '\n\t')}';
    }
    return string;
  }
}

class UnknownBEAMObject implements BEAMSerde {
  const UnknownBEAMObject({this.rawValue});

  final dynamic rawValue;

  @override
  dynamic toJson() => rawValue;

  factory UnknownBEAMObject.fromJson(dynamic json) =>
      UnknownBEAMObject(rawValue: json);
}
