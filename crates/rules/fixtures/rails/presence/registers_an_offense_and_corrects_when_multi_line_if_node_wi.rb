if a.present?
^^^^^^^^^^^^^ Use `a.presence || b.to_f + 12.0` instead of `if a.present? ... end`.
  a
else
  b.to_f + 12.0
end
