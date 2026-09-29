def fixed_point(value)
  if value then value
                ^^^^^ Move `value` out of the conditional.
  else value
       ^^^^^ Move `value` out of the conditional.
  end
end
