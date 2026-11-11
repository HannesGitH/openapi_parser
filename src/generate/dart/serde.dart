// in dart we cannot enforce our classes to have a fromJson method
import 'dart:convert';

abstract interface class BEAMSerde {
  /// either a Map<String, dynamic> or a List<dynamic> or a String or a int or a double or a bool or null
  /// in this case probably only Map<String, dynamic> and String
  dynamic toJson();

  /// Whether this value's raw JSON (what [toJson] returns) structurally
  /// conforms to its declared type. Delegates to the type's static
  /// `checkConform`; [deep] also validates nested objects, enums, unions and
  /// list elements.
  bool conforms({bool deep = true});

  /// Like [conforms], but collects every mismatch (with its JSON path,
  /// starting at [path]) instead of stopping at the first one. Empty iff
  /// [conforms] is true. Delegates to the type's static
  /// `checkConformityErrors`.
  List<BeamConformErr> conformityErrors({String path = r'$', bool deep = true});
}

/// One structural mismatch found by `conformityErrors`.
///
/// Safe to log: [received] never contains the payload value, only its JSON
/// kind — except for an enum (or discriminator) mismatch on a string, which
/// shows the offending string in quotes.
final class BeamConformErr {
  const BeamConformErr({
    required this.identifierPath,
    required this.expected,
    required this.received,
  });

  /// JSON path from the payload root, e.g. `$.data.items[3].amount`.
  final String identifierPath;
  final String expected;
  final String received;

  /// The JSON kind of [value] (never the value itself): `String`, `int`,
  /// `double`, `bool`, `null`, `object` for maps, `List` for lists.
  static String kindOf(dynamic value) => switch (value) {
    null => 'null',
    String() => 'String',
    int() => 'int',
    double() => 'double',
    bool() => 'bool',
    Map() => 'object',
    List() => 'List',
    _ => 'unknown',
  };

  /// A mismatch at [path] whose `received` is the JSON kind of [value].
  factory BeamConformErr.mismatch(String path, String expected, dynamic value) =>
      BeamConformErr(
        identifierPath: path,
        expected: expected,
        received: kindOf(value),
      );

  /// An enum mismatch: a string value is shown quoted (enum values come from
  /// the spec), anything else only by its JSON kind.
  factory BeamConformErr.enumMismatch(
    String path,
    String expected,
    dynamic value,
  ) => BeamConformErr(
    identifierPath: path,
    expected: expected,
    received: value is String ? '"$value"' : kindOf(value),
  );

  @override
  String toString() =>
      'BeamConformErr: $identifierPath expected $expected, received $received';
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

  @override
  bool conforms({bool deep = true}) => checkConform(rawValue, deep: deep);

  @override
  List<BeamConformErr> conformityErrors({
    String path = r'$',
    bool deep = true,
  }) => checkConformityErrors(rawValue, path: path, deep: deep);

  factory UnknownBEAMObject.fromJson(dynamic json) =>
      UnknownBEAMObject(rawValue: json);

  /// `Never`/unknown accepts any JSON, so it always conforms. Present so the
  /// lazy union discriminator can call `checkConform` uniformly on every
  /// `BEAMSerde` value type.
  static bool checkConform(dynamic json, {bool deep = true}) => true;

  static List<BeamConformErr> checkConformityErrors(
    dynamic json, {
    String path = r'$',
    bool deep = true,
  }) => const [];
}
