parsed_params = refusal_advice_params.merge(
  actions: refusal_advice_params.fetch(:actions).
             each_pair do |_, suggestions|
               suggestions.transform_values! { |v| v == 'true' }
             end
).to_h
