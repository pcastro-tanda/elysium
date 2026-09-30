parsed_params = refusal_advice_params.merge(
  actions: refusal_advice_params.fetch(:actions).
    each_pair do |_, suggestions|
    ^^^^^^^^^ Indent `each_pair` 2 spaces more than `refusal_advice_params` on line 2.
      suggestions.transform_values! { |v| v == 'true' }
    end
).to_h
