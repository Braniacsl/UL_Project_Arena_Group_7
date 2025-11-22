import React, { useState, useEffect } from "react";
import "./../static/styles/global.scss";

// Slider Component
const BannerSlider = () => {
  const images = [
    "https://via.placeholder.com/1200x350?text=Banner+1",
    "https://via.placeholder.com/1200x350?text=Banner+2",
    "https://via.placeholder.com/1200x350?text=Banner+3",
  ];

  const [index, setIndex] = useState(0);

  useEffect(() => {
    const interval = setInterval(() => {
      setIndex((prev) => (prev + 1) % images.length);
    }, 3000);

    return () => clearInterval(interval);
  }, []);

  return (
    <div className="banner-slider" style={{borderRadius: "0"}}>
      <img src={images[index]} alt="banner" className="banner-image" />
    </div>
  );
};

// Navbar
const Navbar = () => {
  return (
    <nav className="navbar">
      <div className="navbar-logo" style={{padding: "0 1rem"}}>UL Projects</div>

      <div className="navbar-right " style={{display:"flex", justifyContent: "space-between", padding: "0 1rem", alignItems: "center", gap: "8px"}}>
        <input type="text" placeholder="Search projects..." className="search-input" />

        <div className="notification-wrapper">
          <button className="notification-btn">🔔</button>
          <span className="notification-dot"></span>
        </div>

        <div className="profile-icon">👤</div>
      </div>
    </nav>
  );
};

// Project Card
const ProjectCard = ({ title, liked, date }) => {
  return (
    <div className="project-card">
      <div className="project-title">{title}</div>
      <div className="project-info">
        <span>{liked} ❤️</span>
        <span>{date}</span>
      </div>
    </div>
  );
};

// Grid
const ProjectGrid = () => {
  const sample = [
    { title: "AI Health Assistant", liked: 120, date: "2025-11-21" },
    { title: "Smart Traffic Analyzer", liked: 87, date: "2025-10-10" },
    { title: "UL Companion App", liked: 150, date: "2025-11-18" },
    { title: "Smart Campus Automation", liked: 44, date: "2025-09-05" },
     { title: "AI Health Assistant", liked: 120, date: "2025-11-21" },
    { title: "Smart Traffic Analyzer", liked: 87, date: "2025-10-10" },
    { title: "UL Companion App", liked: 150, date: "2025-11-18" },
    { title: "Smart Campus Automation", liked: 44, date: "2025-09-05" },
  ];

  return (
    <div className="project-grid">
      {sample.map((p, i) => (
        <ProjectCard key={i} {...p} />
      ))}
    </div>
  );
};

// Main Page
export default function HomePage() {
  return (
    <div className="homepage">
      <Navbar />

      <div className="content-wrapper">
        <BannerSlider />
        <h2 className="section-title"  style={{paddingLeft: "2rem"}} >Recent & Popular Projects</h2>
        <div className="projects-section" style={{padding: "2rem"}}>
          <ProjectGrid />
        </div>
      </div>
    </div>
  );
}