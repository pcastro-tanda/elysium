around do |example|
  travel_to(Time.current, &example)
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `freeze_time` instead of `travel_to`.
end
