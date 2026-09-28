x.to_enum(:foreach, path).select { |e| e.file? }.map { |e| [e, name(e)] }.each { |e, p| e.extract(p) }.to_h { |k, v| [k.to_sym, v] }
