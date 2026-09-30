@errors << if var.any?(:prob_a_check)
  'Problem A'
elsif var.any?(:prob_a_check)
  'Problem B'
else
  if var.all?(:save)
    'Save failed'
  else
    'Other'
  end
end
