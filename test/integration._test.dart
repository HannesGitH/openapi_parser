import 'package:test/test.dart';

import '../out/endpoints/endpoints.dart';
import '../out/endpoints/routes/_v1_service_spec_example__id_Methods/patch.req.body.schema.dart';

class TestHandler implements JsonRequestHandler {
  @override
  Future<dynamic> handle({
    dynamic body,
    required APIRequestMethod method,
    Map<String, String> params = const {},
    required String path,
  }) async {
    print(body);
    print(method);
    print(params);
    print(path);
  }
}

void main() {
  group('API works', () {
    // test('v2/cards/options/{cardId}/lock', () {
    //   final api = API(handler: TestHandler()).fragmented;
    //   final card = api.v2.cards.options.cardId('123').lock().post();
    //   print(card);
    // });
    test('v1/service-spec-example/{id}', () {
      final api = API(handler: TestHandler()).fragmented;
      final example = api.v1.service_spec_example.id('123')().patch((bar: '2'),
          body: API_v1_service_spec_example__id_MethodspatchRequestModel(
              foo: '1'));
      print(example);
    });
  });
}
