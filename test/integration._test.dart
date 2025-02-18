import 'package:test/test.dart';

import '../out/endpoints/endpoints.dart';

void main() {
  group('API works', () {
    test('', () {
      final api = API().fragmented;
      final card = api.v2.cards.options.cardId('123').lock.post();
      print(card);
    });
  });
}
