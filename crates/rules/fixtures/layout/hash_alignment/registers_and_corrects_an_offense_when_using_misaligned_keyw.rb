config.fog_credentials_as_kwargs(
  provider:              'AWS',
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Align the keys of a hash literal if they span more than one line.
  aws_access_key_id:     ENV['S3_ACCESS_KEY'],
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Align the keys of a hash literal if they span more than one line.
  aws_secret_access_key: ENV['S3_SECRET'],
  region:                ENV['S3_REGION'],
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Align the keys of a hash literal if they span more than one line.
)
