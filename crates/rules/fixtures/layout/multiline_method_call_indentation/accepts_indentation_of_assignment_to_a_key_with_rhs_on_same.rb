a[:key] = int_part
  .abs
  .to_s
  .reverse
  .gsub(/...(?=.)/, '&_')
  .reverse
