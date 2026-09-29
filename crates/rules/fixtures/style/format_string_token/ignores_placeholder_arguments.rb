format(
  '%<day>s %<start>s-%<end>s',
  day: open_house.starts_at.strftime('%a'),
  start: open_house.starts_at.strftime('%l'),
  end: open_house.ends_at.strftime('%l %p').strip
)
