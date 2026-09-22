Rails.application.config.ideal_postcodes_key =
  if Rails.env.production? || Rails.env.staging?
    "AAAA-AAAA-AAAA-AAAA"
  end
