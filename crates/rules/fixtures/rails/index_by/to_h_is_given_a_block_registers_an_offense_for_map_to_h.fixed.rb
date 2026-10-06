x.index_by { |el| el.to_sym }.to_h { |k, v| [v, k] }
