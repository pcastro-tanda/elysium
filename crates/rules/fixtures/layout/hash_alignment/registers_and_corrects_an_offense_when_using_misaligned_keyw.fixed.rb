config.fog_credentials_as_kwargs(
  provider: 'AWS',
  aws_access_key_id: ENV['S3_ACCESS_KEY'],
  aws_secret_access_key: ENV['S3_SECRET'],
  region: ENV['S3_REGION'],
)
