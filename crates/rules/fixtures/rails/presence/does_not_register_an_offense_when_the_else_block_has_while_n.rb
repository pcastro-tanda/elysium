if a.present?
  a
else
  fetch_state while waiting?
end
