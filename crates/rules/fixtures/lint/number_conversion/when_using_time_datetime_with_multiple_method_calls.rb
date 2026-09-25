Time.now.to_datetime.to_i
DateTime.civil(2005, 2, 21, 10, 11, 12, Rational(-6, 24)).utc.to_f
Time.zone.now.to_datetime.to_f
DateTime.new(2012, 8, 29, 22, 35, 0)
        .change(day: 1)
        .change(month: 1)
        .change(year: 2020)
        .to_i
