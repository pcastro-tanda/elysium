parsed_params = refusal_advice_params.merge(
  actions: refusal_advice_params.fetch(:actions).
      each_pair do |_, suggestions|
      ^^^^^^^^^ Use 2 (not 4) spaces for indenting an expression in an assignment spanning multiple lines.
        suggestions.transform_values! { |v| v == 'true' }
      end
).to_h
