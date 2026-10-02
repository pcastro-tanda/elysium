if something_method.present?
  something_method
else
  invalid_method rescue StandardError
end
