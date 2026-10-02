!Rails.env.development? && Rails.env.production?
!Rails.env.test? && Rails.env.production?
!Rails.env.production? && Rails.env.other?
