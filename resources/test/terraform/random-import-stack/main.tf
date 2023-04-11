terraform {
  required_providers {
    random = {
      source  = "hashicorp/random"
      version = "3.4.3"
    }
    aws = {
      source  = "hashicorp/aws"
      version = "4.62.0"
    }
  }
}

provider "random" {
  # Configuration options
}
provider "aws" {
  # Configuration options
}
