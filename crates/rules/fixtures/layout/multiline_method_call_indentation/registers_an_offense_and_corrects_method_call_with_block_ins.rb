parsed_params = refusal_advice_params.merge(
  actions: refusal_advice_params.fetch(:actions).
              each_pair do |_, suggestions|
              ^^^^^^^^^ Align `each_pair` with `refusal_advice_params.fetch(:actions).` on line 2.
                suggestions.transform_values! { |v| v == 'true' }
              end
).to_h
