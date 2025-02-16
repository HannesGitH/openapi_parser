import 'package:test/test.dart';
import '../out/schemes/CardsOptionsGetRespBody/limits.dart';
import '../out/schemes/CardsOptionsGetRespBody/CardsOptionsGetRespBody_limits/atmMonthly.dart';
import '../out/schemes/CardsOptionsGetRespBody/CardsOptionsGetRespBody_limits/atmWeekly.dart';
import '../out/schemes/CardsOptionsGetRespBody/CardsOptionsGetRespBody_limits/paymentPerTransaction.dart';
import '../out/schemes/CardsOptionsGetRespBody/CardsOptionsGetRespBody_limits/paymentWeekly.dart';

void main() {
  group('CardsOptionsGetRespBody_limitsModel', () {
    test('serializes and deserializes correctly', () {
      final model = APICardsOptionsGetRespBody_limitsModel(
        atmMonthly: APICardsOptionsGetRespBody_limits_atmMonthlyModel(
          configuredLimit: 1000,
          amountUsed: 500,
          maximumLimit: 2000,
        ),
        atmWeekly: APICardsOptionsGetRespBody_limits_atmWeeklyModel(
          configuredLimit: 500,
          amountUsed: 200,
          maximumLimit: 1000,
        ),
        paymentPerTransaction:
            APICardsOptionsGetRespBody_limits_paymentPerTransactionModel(
          configuredLimit: 200,
          maximumLimit: 500,
        ),
        paymentWeekly: APICardsOptionsGetRespBody_limits_paymentWeeklyModel(
          configuredLimit: 1000,
          amountUsed: 750,
          maximumLimit: 2000,
        ),
      );

      final json = model.toJson();
      final deserialized =
          APICardsOptionsGetRespBody_limitsModel.fromJson(json);

      expect(deserialized.atmMonthly.configuredLimit, equals(1000));
      expect(deserialized.atmMonthly.amountUsed, equals(500));
      expect(deserialized.atmMonthly.maximumLimit, equals(2000));

      expect(deserialized.atmWeekly.configuredLimit, equals(500));
      expect(deserialized.atmWeekly.amountUsed, equals(200));
      expect(deserialized.atmWeekly.maximumLimit, equals(1000));

      expect(deserialized.paymentPerTransaction.configuredLimit, equals(200));
      expect(deserialized.paymentPerTransaction.maximumLimit, equals(500));

      expect(deserialized.paymentWeekly.configuredLimit, equals(1000));
      expect(deserialized.paymentWeekly.amountUsed, equals(750));
      expect(deserialized.paymentWeekly.maximumLimit, equals(2000));
    });
  });
}
