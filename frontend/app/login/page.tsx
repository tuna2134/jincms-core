"use client";
import React from "react";

export default function Page() {
  React.useEffect(() => {
    const checkLink = async () => {
      const res = await fetch(
        `${process.env.NEXT_PUBLIC_API_ENDPOINT}/users/oauth_url`,
      );
      const data = await res.json();
      window.location.href = data.url;
    };
    checkLink();
  });
  return <p>ページ転移します。</p>;
}
