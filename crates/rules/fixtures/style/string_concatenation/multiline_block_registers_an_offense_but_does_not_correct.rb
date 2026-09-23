'(' + values.map do |v|
^^^^^^^^^^^^^^^^^^^^^^^ Prefer string interpolation to string concatenation.
    v.titleize
end.join(', ') + ')'
